mod commands;
mod config;
mod history;
mod plugins;
mod watcher;

use commands::AppState;
use config::load_config;
use history::HistoryStore;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex as StdMutex;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, RunEvent, WindowEvent,
};
use watcher::ClipboardWatcher;

#[cfg(target_os = "macos")]
use tauri::ActivationPolicy;

const TRAY_ICON_PNG: &[u8] = include_bytes!("../icons/trayicon.png");
/// Full color app icon for Dock re-application after Accessory policy.
const APP_ICON_PNG: &[u8] = include_bytes!("../icons/appicon.png");

#[allow(dead_code)]
struct TrayHandle(tauri::tray::TrayIcon);
#[allow(dead_code)]
struct WatcherHandle(ClipboardWatcher);

struct Lifecycle {
    quitting: AtomicBool,
}

impl Lifecycle {
    fn new() -> Self {
        Self {
            quitting: AtomicBool::new(false),
        }
    }

    fn request_quit(&self, app: &AppHandle) {
        self.quitting.store(true, Ordering::SeqCst);
        app.exit(0);
    }

    fn is_quitting(&self) -> bool {
        self.quitting.load(Ordering::SeqCst)
    }
}

/// Load app icon as NSImage (must run on main thread).
#[cfg(target_os = "macos")]
fn load_app_ns_image() -> Option<objc2::rc::Retained<objc2_app_kit::NSImage>> {
    use objc2::AnyThread;
    use objc2_app_kit::NSImage;
    use objc2_foundation::NSData;

    let data = NSData::with_bytes(APP_ICON_PNG);
    NSImage::initWithData(NSImage::alloc(), &data)
}

/// Re-apply Dock/application icon after activation policy flips.
/// Without this, macOS often falls back to a generic `exec`/binary icon in dev and after Accessory.
#[cfg(target_os = "macos")]
fn reapply_app_icon() {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApplication;

    let mtm = MainThreadMarker::new().unwrap_or_else(|| unsafe { MainThreadMarker::new_unchecked() });
    let Some(app_icon) = load_app_ns_image() else {
        return;
    };
    let app = NSApplication::sharedApplication(mtm);
    unsafe {
        app.setApplicationIconImage(Some(&app_icon));
    }
}

#[cfg(not(target_os = "macos"))]
fn reapply_app_icon() {}

/// Activate the process so our window can come above other apps (hotkey path).
#[cfg(target_os = "macos")]
fn activate_app_ignoring_other_apps() {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApplication;

    let mtm = MainThreadMarker::new().unwrap_or_else(|| unsafe { MainThreadMarker::new_unchecked() });
    let app = NSApplication::sharedApplication(mtm);
    // Prefer modern API; keep ignoring-other-apps for reliable hotkey raise.
    app.activate();
    #[allow(deprecated)]
    app.activateIgnoringOtherApps(true);
}

/// Order the NSWindow to the front of the window list.
#[cfg(target_os = "macos")]
fn order_window_front(window: &tauri::WebviewWindow) {
    if let Ok(ns_ptr) = window.ns_window() {
        if !ns_ptr.is_null() {
            use objc2_app_kit::NSWindow;
            // SAFETY: pointer from Tauri is a valid NSWindow*.
            let ns_window = unsafe { &*(ns_ptr as *const NSWindow) };
            ns_window.makeKeyAndOrderFront(None);
            ns_window.orderFrontRegardless();
        }
    }
}

/// Hide main window and Dock icon; keep tray only.
pub fn hide_to_tray(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = app.set_activation_policy(ActivationPolicy::Accessory);
        let _ = app.set_dock_visibility(false);
    }
}

/// Show main window, restore Dock, and bring panel to the frontmost layer.
pub fn show_main(app: &AppHandle) {
    let app = app.clone();
    let app_for_thread = app.clone();
    let _ = app.run_on_main_thread(move || {
        #[cfg(target_os = "macos")]
        {
            let _ = app_for_thread.set_activation_policy(ActivationPolicy::Regular);
            let _ = app_for_thread.set_dock_visibility(true);
            reapply_app_icon();
            // Ensure process is active so the window can order front over other apps.
            activate_app_ignoring_other_apps();
        }

        if let Some(window) = app_for_thread.get_webview_window("main") {
            if let Ok(icon) = Image::from_bytes(APP_ICON_PNG) {
                let _ = window.set_icon(icon);
            }
            let _ = window.unminimize();
            let _ = window.show();
            // Raise + focus: call twice around a short re-order for stubborn front windows.
            let _ = window.set_focus();
            #[cfg(target_os = "macos")]
            {
                order_window_front(&window);
            }
            let _ = window.set_focus();
        }

        // Icon can lag one tick after policy change — apply again shortly after.
        #[cfg(target_os = "macos")]
        {
            let app2 = app_for_thread.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(80));
                let app3 = app2.clone();
                let _ = app2.run_on_main_thread(move || {
                    reapply_app_icon();
                    activate_app_ignoring_other_apps();
                    if let Some(window) = app3.get_webview_window("main") {
                        order_window_front(&window);
                        let _ = window.set_focus();
                    }
                });
            });
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let data_root = HistoryStore::data_root().expect("resolve NfunCv data root");
    std::fs::create_dir_all(&data_root).expect("create data root");
    let app_config = load_config(&data_root).unwrap_or_default();
    let store =
        HistoryStore::open(&data_root, app_config.history.max_items).expect("open history store");
    let poll_ms = app_config.poll_interval_ms;
    let toggle_hotkey = app_config.toggle_hotkey.clone();

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(AppState {
            store: Mutex::new(store),
            data_root: data_root.clone(),
            registered_hotkeys: StdMutex::new(Vec::new()),
        })
        .manage(Lifecycle::new())
        .setup(move |app| {
            let settings_i = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&settings_i, &quit_i])?;

            let tray_icon = Image::from_bytes(TRAY_ICON_PNG).unwrap_or_else(|_| {
                app.default_window_icon()
                    .cloned()
                    .expect("default window icon")
            });

            // Left-click shows window; right-click opens menu (设置 / 退出).
            let mut tray_builder = TrayIconBuilder::with_id("main-tray")
                .icon(tray_icon)
                .menu(&menu)
                .tooltip("FunCV")
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "settings" => {
                        show_main(app);
                        let _ = app.emit("open-settings", ());
                    }
                    "quit" => {
                        if let Some(life) = app.try_state::<Lifecycle>() {
                            life.request_quit(app);
                        } else {
                            app.exit(0);
                        }
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        // Click tray → show main window immediately.
                        show_main(tray.app_handle());
                    }
                });

            #[cfg(target_os = "macos")]
            {
                tray_builder = tray_builder.icon_as_template(true);
            }

            let tray = tray_builder.build(app)?;
            app.manage(TrayHandle(tray));

            if let Some(window) = app.get_webview_window("main") {
                // Set icon asynchronously so setup returns quickly and UI can interact.
                if let Ok(icon) = Image::from_bytes(APP_ICON_PNG) {
                    let _ = window.set_icon(icon);
                }
                let handle = app.handle().clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        hide_to_tray(&handle);
                    }
                });
            }

            // Defer Dock icon reapply — avoid contending main thread during first interaction.
            #[cfg(target_os = "macos")]
            {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(120));
                    let _ = handle.run_on_main_thread(|| {
                        reapply_app_icon();
                    });
                });
            }

            let state = app.state::<AppState>();
            if let Err(err) =
                commands::register_toggle_hotkey(app.handle(), &state, &toggle_hotkey)
            {
                eprintln!("[nfun-cv] register toggle hotkey failed: {err}");
            }

            // Watcher starts delayed + skips baseline clipboard (see watcher.rs).
            let watcher = ClipboardWatcher::start(app.handle().clone(), poll_ms);
            app.manage(WatcherHandle(watcher));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_history,
            commands::delete_history,
            commands::clear_history,
            commands::get_history_item,
            commands::copy_history,
            commands::resolve_image_path,
            commands::get_config,
            commands::save_app_config,
            commands::data_root_path,
            commands::hide_main_window,
            commands::show_main_window,
            commands::list_plugins,
            commands::set_plugin_enabled,
            commands::import_plugin,
            commands::remove_plugin,
            commands::run_plugin,
            commands::run_enabled_plugins,
            commands::ecdict_status,
            commands::install_ecdict,
            commands::plugin_protocol_help,
            commands::list_plugin_samples,
            commands::export_plugin_sample,
            commands::export_all_plugin_samples,
        ])
        .build(tauri::generate_context!())
        .expect("error while building FunCV");

    app.run(|app_handle, event| {
        match event {
            RunEvent::Ready => {
                #[cfg(target_os = "macos")]
                {
                    let handle = app_handle.clone();
                    let _ = handle.run_on_main_thread(|| {
                        reapply_app_icon();
                    });
                }
            }
            RunEvent::ExitRequested { api, code, .. } => {
                let quitting = app_handle
                    .try_state::<Lifecycle>()
                    .map(|l| l.is_quitting())
                    .unwrap_or(false);
                if code.is_none() && !quitting {
                    api.prevent_exit();
                }
            }
            #[cfg(target_os = "macos")]
            RunEvent::Reopen {
                has_visible_windows,
                ..
            } => {
                if !has_visible_windows {
                    show_main(app_handle);
                }
            }
            _ => {}
        }
    });
}
