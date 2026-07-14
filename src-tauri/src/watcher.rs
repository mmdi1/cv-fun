use crate::history::ItemKind;
use arboard::Clipboard;
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

/// Watches the system clipboard by polling (not keyboard).
///
/// macOS: uses `NSPasteboard.changeCount` so we only read content when the
/// pasteboard actually changes — avoids multi-second image reads blocking
/// detection of new text copies.
pub struct ClipboardWatcher {
    stop: Arc<AtomicBool>,
}

impl ClipboardWatcher {
    pub fn start(
        app: AppHandle,
        interval_ms: u64,
        watch_text: Arc<AtomicBool>,
        watch_image: Arc<AtomicBool>,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = Arc::clone(&stop);
        let interval = Duration::from_millis(interval_ms.max(150));

        thread::spawn(move || {
            // Wait for UI to become interactive before touching the pasteboard.
            // Opening NSPasteboard / decoding a large image can freeze early frames.
            thread::sleep(Duration::from_millis(900));

            let mut clipboard = match Clipboard::new() {
                Ok(c) => c,
                Err(err) => {
                    eprintln!("[nfun-cv] clipboard open failed: {err}");
                    return;
                }
            };

            // Baseline only — never import whatever is already on the clipboard at launch.
            // Prefer changeCount so we do NOT decode images during prime.
            let mut last_change: Option<i64> = pasteboard_change_count();
            let mut last_content_sig: Option<String> = None;
            let mut primed = last_change.is_some();

            // Fallback platforms without changeCount: only hash text (skip get_image on prime).
            if !primed {
                if watch_text.load(Ordering::SeqCst) {
                    if let Ok(text) = clipboard.get_text() {
                        let t = text.trim();
                        if !t.is_empty() {
                            let mut hasher = Sha256::new();
                            hasher.update(b"text:");
                            hasher.update(t.as_bytes());
                            last_content_sig =
                                Some(format!("txt:{}", hex::encode(hasher.finalize())));
                        }
                    }
                }
                primed = true;
            }

            eprintln!(
                "[nfun-cv] clipboard watcher ready (changeCount={:?}, interval={}ms, text={} image={})",
                last_change,
                interval.as_millis(),
                watch_text.load(Ordering::SeqCst),
                watch_image.load(Ordering::SeqCst),
            );

            while !stop_flag.load(Ordering::SeqCst) {
                let wt = watch_text.load(Ordering::SeqCst);
                let wi = watch_image.load(Ordering::SeqCst);
                // Both off should not happen (sanitize forces text); skip work if so.
                if !wt && !wi {
                    thread::sleep(interval);
                    continue;
                }

                // --- Cheap path: generation counter (macOS) ---
                if let Some(count) = pasteboard_change_count() {
                    if last_change == Some(count) {
                        thread::sleep(interval);
                        continue;
                    }
                    last_change = Some(count);

                    if !primed {
                        primed = true;
                        // Baseline generation only.
                        thread::sleep(interval);
                        continue;
                    }

                    match read_snapshot(&mut clipboard, wt, wi) {
                        Ok(Some(snapshot)) => {
                            // Deduplicate identical content even if changeCount moved
                            // (e.g. app re-wrote the same text).
                            if last_content_sig.as_ref() == Some(&snapshot.signature) {
                                thread::sleep(interval);
                                continue;
                            }
                            last_content_sig = Some(snapshot.signature.clone());
                            if let Err(err) = commit_snapshot(&app, snapshot) {
                                eprintln!("[nfun-cv] history commit failed: {err}");
                            }
                        }
                        Ok(None) => {
                            // Cleared / unsupported formats — keep watching.
                        }
                        Err(err) => {
                            eprintln!("[nfun-cv] clipboard read failed: {err}");
                            // Recreate clipboard handle after errors.
                            if let Ok(c) = Clipboard::new() {
                                clipboard = c;
                            }
                        }
                    }
                    thread::sleep(interval);
                    continue;
                }

                // --- Fallback: content fingerprint every poll ---
                match read_snapshot(&mut clipboard, wt, wi) {
                    Ok(Some(snapshot)) => {
                        if last_content_sig.as_ref() != Some(&snapshot.signature) {
                            let sig = snapshot.signature.clone();
                            if !primed {
                                primed = true;
                                last_content_sig = Some(sig);
                            } else {
                                last_content_sig = Some(sig);
                                if let Err(err) = commit_snapshot(&app, snapshot) {
                                    eprintln!("[nfun-cv] history commit failed: {err}");
                                }
                            }
                        }
                    }
                    Ok(None) => {}
                    Err(err) => {
                        eprintln!("[nfun-cv] clipboard read failed: {err}");
                        if let Ok(c) = Clipboard::new() {
                            clipboard = c;
                        }
                    }
                }
                thread::sleep(interval);
            }
        });

        Self { stop }
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

impl Drop for ClipboardWatcher {
    fn drop(&mut self) {
        self.stop();
    }
}

struct Snapshot {
    signature: String,
    text: Option<String>,
    image: Option<OwnedImage>,
}

struct OwnedImage {
    width: u32,
    height: u32,
    bytes: Vec<u8>,
}

/// macOS pasteboard generation counter. `None` on other platforms / failure.
#[cfg(target_os = "macos")]
fn pasteboard_change_count() -> Option<i64> {
    use objc2_app_kit::NSPasteboard;
    // generalPasteboard is safe to call from background threads for changeCount.
    let pb = NSPasteboard::generalPasteboard();
    Some(pb.changeCount() as i64)
}

#[cfg(not(target_os = "macos"))]
fn pasteboard_change_count() -> Option<i64> {
    None
}

fn read_snapshot(
    clipboard: &mut Clipboard,
    watch_text: bool,
    watch_image: bool,
) -> Result<Option<Snapshot>, String> {
    // Text first when enabled — cheap, and preferred when both text and image are present.
    let text = if watch_text {
        match clipboard.get_text() {
            Ok(t) => {
                let t = t.trim().to_string();
                if t.is_empty() {
                    None
                } else {
                    Some(t)
                }
            }
            Err(_) => None,
        }
    } else {
        None
    };

    // Only decode image when watching images. If text is also watched and present,
    // skip image (same preference as before). When text watching is off, read image
    // even if pasteboard also has text.
    let image = if watch_image && (text.is_none() || !watch_text) {
        match clipboard.get_image() {
            Ok(img) => {
                let bytes = img.bytes.into_owned();
                if bytes.is_empty() || img.width == 0 || img.height == 0 {
                    None
                } else {
                    Some(OwnedImage {
                        width: img.width as u32,
                        height: img.height as u32,
                        bytes,
                    })
                }
            }
            Err(_) => None,
        }
    } else {
        None
    };

    // Drop types that are not enabled (defensive)
    let text = if watch_text { text } else { None };
    let image = if watch_image { image } else { None };

    if text.is_none() && image.is_none() {
        return Ok(None);
    }

    let signature = if let Some(ref t) = text {
        format!("txt:{}", hash_bytes(t.as_bytes()))
    } else if let Some(ref img) = image {
        light_image_signature(img)
    } else {
        return Ok(None);
    };

    Ok(Some(Snapshot {
        signature,
        text,
        image,
    }))
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn light_image_signature(img: &OwnedImage) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"image:");
    hasher.update(img.width.to_le_bytes());
    hasher.update(img.height.to_le_bytes());
    hasher.update((img.bytes.len() as u64).to_le_bytes());
    let n = img.bytes.len();
    if n == 0 {
        return format!("img:{}", hex::encode(hasher.finalize()));
    }
    let chunk = 4096.min(n);
    hasher.update(&img.bytes[..chunk]);
    if n > chunk * 2 {
        let mid = n / 2;
        let end = (mid + chunk).min(n);
        hasher.update(&img.bytes[mid..end]);
        hasher.update(&img.bytes[n - chunk..]);
    }
    format!("img:{}", hex::encode(hasher.finalize()))
}

fn commit_snapshot(app: &AppHandle, snapshot: Snapshot) -> Result<(), String> {
    let state = app
        .try_state::<crate::commands::AppState>()
        .ok_or_else(|| "app state missing".to_string())?;

    let added = {
        let store = state.store.lock();
        if let Some(img) = snapshot.image {
            store
                .add_image(&img.bytes, img.width, img.height)
                .map_err(|e| e.to_string())?
        } else if let Some(text) = snapshot.text {
            store.add_text(&text).map_err(|e| e.to_string())?
        } else {
            return Ok(());
        }
    };
    let item = added.item;

    let kind = match item.kind {
        ItemKind::Text => "text",
        ItemKind::Image => "image",
    };
    eprintln!(
        "[nfun-cv] clipboard captured id={} kind={} dup={} consec={} preview={}",
        item.id, kind, added.is_duplicate, added.is_consecutive, item.preview
    );

    let payload = serde_json::json!({
        "id": item.id,
        "kind": kind,
        "duplicate": added.is_duplicate,
        "consecutive": added.is_consecutive,
    });

    // Prefer window emit so the webview definitely receives the event.
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.emit("clipboard-history-changed", &payload);
        let _ = window.emit("clipboard-stats-changed", &payload);
    } else {
        let _ = app.emit("clipboard-history-changed", &payload);
        let _ = app.emit("clipboard-stats-changed", &payload);
    }
    Ok(())
}
