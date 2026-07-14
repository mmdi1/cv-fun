//! Local image OCR.
//! - macOS: Apple Vision (system, no model download)
//! - Windows: Windows.Media.Ocr via PowerShell WinRT (built-in capability)

use std::path::Path;
#[cfg(target_os = "windows")]
use std::process::Command;

/// Recognize text in an image file on disk.
pub fn ocr_image_file(path: &Path) -> Result<String, String> {
    if !path.exists() {
        return Err(format!("图片不存在: {}", path.display()));
    }

    #[cfg(target_os = "macos")]
    {
        return macos_ocr(path);
    }

    #[cfg(target_os = "windows")]
    {
        return windows_ocr(path);
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = path;
        Err("当前平台暂不支持本地图片 OCR".into())
    }
}

#[cfg(target_os = "macos")]
fn macos_ocr(path: &Path) -> Result<String, String> {
    use std::ffi::{CStr, CString};
    use std::os::raw::c_char;

    extern "C" {
        fn nfun_ocr_image_at_path(path_utf8: *const c_char, out_err: *mut *mut c_char)
            -> *mut c_char;
        fn nfun_ocr_free(s: *mut c_char);
    }

    let c_path = CString::new(path.to_string_lossy().as_bytes())
        .map_err(|_| "图片路径包含无效字符".to_string())?;

    let mut err_ptr: *mut c_char = std::ptr::null_mut();
    let text_ptr = unsafe { nfun_ocr_image_at_path(c_path.as_ptr(), &mut err_ptr) };

    if text_ptr.is_null() {
        let msg = if !err_ptr.is_null() {
            let s = unsafe { CStr::from_ptr(err_ptr) }
                .to_string_lossy()
                .into_owned();
            unsafe { nfun_ocr_free(err_ptr) };
            s
        } else {
            "Vision OCR 失败".into()
        };
        return Err(msg);
    }

    let text = unsafe { CStr::from_ptr(text_ptr) }
        .to_string_lossy()
        .into_owned();
    unsafe { nfun_ocr_free(text_ptr) };
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err("未识别到文字".into());
    }
    Ok(text)
}

/// Windows.Media.Ocr via PowerShell + WinRT (no extra install on Windows 10+).
#[cfg(target_os = "windows")]
fn windows_ocr(path: &Path) -> Result<String, String> {
    let path_str = path
        .canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_string();
    // Escape for single-quoted PowerShell string
    let escaped = path_str.replace('\'', "''");

    let script = format!(
        r#"
$ErrorActionPreference = 'Stop'
$path = '{escaped}'
Add-Type -AssemblyName System.Runtime.WindowsRuntime | Out-Null
$null = [Windows.Storage.StorageFile,Windows.Storage,ContentType=WindowsRuntime]
$null = [Windows.Media.Ocr.OcrEngine,Windows.Foundation,ContentType=WindowsRuntime]
$null = [Windows.Graphics.Imaging.BitmapDecoder,Windows.Foundation,ContentType=WindowsRuntime]
$null = [Windows.Storage.Streams.RandomAccessStream,Windows.Storage.Streams,ContentType=WindowsRuntime]

function Await($WinRtTask, $ResultType) {{
  $asTaskGeneric = ([System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object {{
    $_.Name -eq 'AsTask' -and $_.GetParameters().Count -eq 1 -and $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperation`1'
  }})[0]
  $asTask = $asTaskGeneric.MakeGenericMethod($ResultType)
  $netTask = $asTask.Invoke($null, @($WinRtTask))
  $netTask.Wait(-1) | Out-Null
  $netTask.Result
}}

try {{
  $file = Await ([Windows.Storage.StorageFile]::GetFileFromPathAsync($path)) ([Windows.Storage.StorageFile])
  $stream = Await ($file.OpenAsync([Windows.Storage.FileAccessMode]::Read)) ([Windows.Storage.Streams.IRandomAccessStream])
  $decoder = Await ([Windows.Graphics.Imaging.BitmapDecoder]::CreateAsync($stream)) ([Windows.Graphics.Imaging.BitmapDecoder])
  $bitmap = Await ($decoder.GetSoftwareBitmapAsync()) ([Windows.Graphics.Imaging.SoftwareBitmap])
  $engine = [Windows.Media.Ocr.OcrEngine]::TryCreateFromUserProfileLanguages()
  if ($null -eq $engine) {{
    Write-Error '无法创建 OCR 引擎（请确认系统语言包支持 OCR）'
  }}
  $result = Await ($engine.RecognizeAsync($bitmap)) ([Windows.Media.Ocr.OcrResult])
  if ([string]::IsNullOrWhiteSpace($result.Text)) {{
    Write-Error '未识别到文字'
  }}
  [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
  Write-Output $result.Text
}} catch {{
  [Console]::Error.WriteLine($_.Exception.Message)
  exit 1
}}
"#
    );

    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ])
        .output()
        .map_err(|e| format!("启动 PowerShell 失败: {e}"))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        let err = err.trim();
        let out = String::from_utf8_lossy(&output.stdout);
        let out = out.trim();
        let msg = if !err.is_empty() {
            err.to_string()
        } else if !out.is_empty() {
            out.to_string()
        } else {
            "Windows OCR 失败".into()
        };
        return Err(msg);
    }

    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        return Err("未识别到文字".into());
    }
    Ok(text)
}
