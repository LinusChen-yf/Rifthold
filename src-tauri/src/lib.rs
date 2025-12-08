use std::sync::{Arc, Mutex, atomic::{AtomicU64, Ordering}};
use std::fs;
use std::path::PathBuf;

use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, Runtime, State, WebviewWindow,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Config {
    shortcut: String,
}

fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("rifthold")
        .join("config.toml")
}

fn load_config() -> Config {
    if let Ok(content) = fs::read_to_string(config_path()) {
        toml::from_str(&content).unwrap_or_else(|_| Config { shortcut: "alt+space".into() })
    } else {
        Config { shortcut: "alt+space".into() }
    }
}

fn save_config(config: &Config) -> Result<(), String> {
    let path = config_path();
    fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    let content = toml::to_string(config).map_err(|e| e.to_string())?;
    fs::write(path, content).map_err(|e| e.to_string())
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WindowInfo {
    pub id: String,
    pub title: String,
    pub app_name: String,
    pub is_title_fallback: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
}

trait WindowProvider: Send + Sync {
    fn list(&self, capture_thumbnails: bool) -> Vec<WindowInfo>;
    fn activate(&self, id: &str) -> Result<(), String>;
    fn clear_cache(&self);
}

#[cfg(not(target_os = "macos"))]
#[derive(Default)]
struct MockWindowProvider;

#[cfg(not(target_os = "macos"))]
impl WindowProvider for MockWindowProvider {
    fn list(&self, _capture_thumbnails: bool) -> Vec<WindowInfo> {
        vec![
            WindowInfo {
                id: "1".into(),
                title: "Mock Window — code editor".into(),
                app_name: "VS Code".into(),
                is_title_fallback: false,
                thumbnail: None,
            },
            WindowInfo {
                id: "2".into(),
                title: "Mock Window — product specs".into(),
                app_name: "Notion".into(),
                is_title_fallback: false,
                thumbnail: None,
            },
            WindowInfo {
                id: "3".into(),
                title: "Mock Window — design board".into(),
                app_name: "Figma".into(),
                is_title_fallback: false,
                thumbnail: None,
            },
            WindowInfo {
                id: "4".into(),
                title: "Mock Window — browser".into(),
                app_name: "Arc".into(),
                is_title_fallback: false,
                thumbnail: None,
            },
        ]
    }

    fn activate(&self, id: &str) -> Result<(), String> {
        println!("activate_window called with id={}", id);
        Ok(())
    }

    fn clear_cache(&self) {
        // No-op for mock provider
    }
}

struct WindowService {
    provider: Arc<dyn WindowProvider>,
}

struct ShortcutConfig {
    current: Mutex<String>,
}

/// Counter to cancel stale refresh requests
static REFRESH_GENERATION: AtomicU64 = AtomicU64::new(0);

impl WindowService {
    fn new(provider: Arc<dyn WindowProvider>) -> Self {
        Self { provider }
    }

    fn list(&self, capture_thumbnails: bool) -> Vec<WindowInfo> {
        self.provider.list(capture_thumbnails)
    }

    fn activate(&self, id: &str) -> Result<(), String> {
        self.provider.activate(id)
    }

    fn clear_cache(&self) {
        self.provider.clear_cache()
    }
}

fn build_provider() -> Arc<dyn WindowProvider> {
    #[cfg(target_os = "macos")]
    {
        Arc::new(macos::MacWindowProvider::new())
    }

    #[cfg(not(target_os = "macos"))]
    {
        Arc::new(MockWindowProvider::default())
    }
}

#[tauri::command]
fn list_windows(
    service: State<WindowService>,
    refresh_cache: Option<bool>,
    capture_thumbnails: Option<bool>,
) -> Vec<WindowInfo> {
    let refresh = refresh_cache.unwrap_or(false);
    let capture = capture_thumbnails.unwrap_or(true);

    println!("[list_windows] refresh_cache={:?} (resolved={}), capture_thumbnails={:?} (resolved={})",
        refresh_cache, refresh, capture_thumbnails, capture);

    if refresh {
        service.clear_cache();
    }
    service.list(capture)
}

#[tauri::command]
fn activate_window(
    id: String,
    service: State<WindowService>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    service.activate(&id)?;

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }

    Ok(())
}

#[tauri::command]
fn get_window_thumbnail(window_id: String) -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let id = window_id.parse::<i64>().ok()?;
        macos::capture_window_thumbnail(id, 500)
    }

    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

#[tauri::command]
fn check_screen_recording_permission() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::has_screen_recording_permission()
    }

    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

#[tauri::command]
fn log_debug(msg: String) {
    println!("{}", msg);
}

#[tauri::command]
fn switch_to_english_input() {
    #[cfg(target_os = "macos")]
    {
        macos::switch_to_english_input();
    }
}

#[tauri::command]
fn get_shortcut(config: State<ShortcutConfig>) -> String {
    config.current.lock().unwrap().clone()
}

#[tauri::command]
fn set_shortcut(app: AppHandle, config: State<ShortcutConfig>, shortcut: String) -> Result<(), String> {
    app.global_shortcut().unregister_all().map_err(|e| e.to_string())?;

    let parsed: Shortcut = shortcut.parse().map_err(|e| format!("{:?}", e))?;

    app.global_shortcut()
        .on_shortcut(parsed, move |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                let _ = toggle_overlay(app);
            }
        })
        .map_err(|e| e.to_string())?;

    *config.current.lock().unwrap() = shortcut.clone();
    save_config(&Config { shortcut })?;
    Ok(())
}

#[tauri::command]
async fn refresh_windows_async(app: tauri::AppHandle, service: State<'_, WindowService>) -> Result<(), String> {
    // Increment generation to cancel any in-flight tasks
    let current_gen = REFRESH_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;

    // Clone the provider Arc to move into spawned task
    let provider = service.provider.clone();

    // Spawn the entire refresh operation to avoid blocking the main thread
    tauri::async_runtime::spawn(async move {
        // Check if this request is still current
        if REFRESH_GENERATION.load(Ordering::SeqCst) != current_gen {
            return;
        }

        // Get window list in a blocking task (it calls CoreGraphics APIs)
        let windows = tauri::async_runtime::spawn_blocking(move || {
            provider.list(false)
        }).await.unwrap_or_default();

        // Check again after getting window list
        if REFRESH_GENERATION.load(Ordering::SeqCst) != current_gen {
            println!("[thumbnail] stale after list fetch (gen {})", current_gen);
            return;
        }

        // Emit window list immediately
        let _ = app.emit("windows:list", &windows);

        let batch_start = std::time::Instant::now();

        // Spawn all thumbnail tasks in parallel for maximum speed
        let mut tasks = Vec::with_capacity(windows.len());
        for window in windows.iter() {
            if let Ok(window_id) = window.id.parse::<i64>() {
                let window_id_str = window.id.clone();
                let app_clone = app.clone();

                let task = tauri::async_runtime::spawn_blocking(move || {
                    // Check if still current before doing expensive work
                    if REFRESH_GENERATION.load(Ordering::SeqCst) != current_gen {
                        return;
                    }

                    #[cfg(target_os = "macos")]
                    {
                        if let Some(thumbnail) = macos::capture_window_thumbnail(window_id, 500) {
                            // Check before emitting
                            if REFRESH_GENERATION.load(Ordering::SeqCst) != current_gen {
                                return;
                            }
                            let payload = serde_json::json!({
                                "id": window_id_str,
                                "thumbnail": thumbnail
                            });
                            let _ = app_clone.emit("window:thumbnail", payload);
                        }
                    }
                });
                tasks.push(task);
            }
        }

        // Wait for all tasks (they will self-cancel via generation check)
        for task in tasks {
            let _ = task.await;
        }

        // Only emit completion if this is still the current generation
        if REFRESH_GENERATION.load(Ordering::SeqCst) == current_gen {
            let total_elapsed = batch_start.elapsed().as_millis();
            println!("[thumbnail] batch complete: {} windows in {}ms (gen {})", windows.len(), total_elapsed, current_gen);
            let _ = app.emit("windows:thumbnails-complete", ());
        }
    });

    Ok(())
}

fn fit_to_current_workspace<R: Runtime>(
    app: &AppHandle<R>,
    window: &WebviewWindow<R>,
) -> tauri::Result<()> {
    let monitor = window.current_monitor()?.or(app.primary_monitor()?);
    if let Some(monitor) = monitor {
        let scale = monitor.scale_factor();
        let size = monitor.size().to_logical::<f64>(scale);
        let position = monitor.position().to_logical::<f64>(scale);

        window.set_size(LogicalSize::new(size.width, size.height))?;
        window.set_position(LogicalPosition::new(position.x, position.y))?;
    }
    Ok(())
}

fn focus_overlay<R: Runtime>(app: &AppHandle<R>, window: &WebviewWindow<R>) -> tauri::Result<()> {
    // Show window first for instant visibility
    window.show()?;
    window.unminimize()?;
    window.set_always_on_top(true)?;

    // Then immediately adjust size and position
    fit_to_current_workspace(app, window)?;

    window.set_focus()?;
    Ok(())
}

fn emit_overview_show<R: Runtime>(app: &AppHandle<R>) {
    let _ = app.emit("overview:show", ());
}

fn toggle_overlay<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible()? {
            window.hide()?;
        } else {
            focus_overlay(app, &window)?;
            emit_overview_show(app);
        }
    }
    Ok(())
}

fn register_shortcuts<R: Runtime>(app: &tauri::App<R>) -> tauri::Result<()> {
    #[cfg(target_os = "macos")]
    {
        use cocoa::appkit::{NSApplication, NSApplicationActivationPolicy};
        unsafe {
            let ns_app = cocoa::appkit::NSApp();
            ns_app.setActivationPolicy_(NSApplicationActivationPolicy::NSApplicationActivationPolicyAccessory);
        }
    }

    app.handle().plugin(tauri_plugin_global_shortcut::Builder::new().build())?;

    let config = load_config();
    let shortcut: Shortcut = config.shortcut.parse()
        .map_err(|e| tauri::Error::PluginInitialization("global-shortcut".into(), format!("{:?}", e)))?;

    app.global_shortcut()
        .on_shortcut(shortcut, |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                let _ = toggle_overlay(app);
            }
        })
        .map_err(|e| tauri::Error::PluginInitialization("global-shortcut".into(), e.to_string()))?;

    if let Some(window) = app.get_webview_window("main") {
        let _ = fit_to_current_workspace(&app.handle(), &window);
    }

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let provider = build_provider();
    let config = load_config();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(WindowService::new(provider))
        .manage(ShortcutConfig {
            current: Mutex::new(config.shortcut),
        })
        .invoke_handler(tauri::generate_handler![
            list_windows,
            activate_window,
            get_window_thumbnail,
            refresh_windows_async,
            get_shortcut,
            set_shortcut,
            check_screen_recording_permission,
            switch_to_english_input,
            log_debug
        ])
        .setup(|app| {
            // Warm up the window list API in background to avoid first-call latency
            let provider = app.state::<WindowService>().provider.clone();
            std::thread::spawn(move || {
                let _ = provider.list(false);
                println!("[rifthold] window list API warmed up");
            });
            register_shortcuts(app).map_err(Into::into)
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(target_os = "macos")]
mod macos {
    use super::{WindowInfo, WindowProvider};
    use core_foundation::{
        base::{CFTypeRef, TCFType},
        dictionary::CFDictionary,
        number::CFNumber,
        string::{CFString, CFStringRef},
    };
    use core_graphics::{
        display::CGRect,
        geometry::{CGPoint, CGSize},
        window::{
            create_description_from_array, create_window_list, kCGNullWindowID,
            kCGWindowLayer, kCGWindowListExcludeDesktopElements, kCGWindowListOptionOnScreenOnly,
            kCGWindowName, kCGWindowNumber, kCGWindowOwnerName, kCGWindowOwnerPID,
            kCGWindowImageBoundsIgnoreFraming, kCGWindowImageDefault, kCGWindowListOptionIncludingWindow,
        },
    };
    use cocoa::appkit::{NSApplicationActivateIgnoringOtherApps, NSRunningApplication};
    use cocoa::base::nil;
    use std::{collections::HashMap, process::Command, sync::{Arc, Mutex}, time::Instant};
    use image::ImageEncoder;
    use base64::{Engine as _, engine::general_purpose};
    use rayon::prelude::*;

    #[derive(Clone)]
    struct MacWindowEntry {
        id: String,
        app_name: String,
        title: String,
        is_title_fallback: bool,
        owner_pid: Option<i64>,
    }

    pub struct MacWindowProvider {
        snapshot: Arc<Mutex<HashMap<String, MacWindowEntry>>>,
    }

    impl MacWindowProvider {
        pub fn new() -> Self {
            Self {
                snapshot: Arc::new(Mutex::new(HashMap::new())),
            }
        }

        fn refresh_snapshot(&self, entries: &[MacWindowEntry]) {
            let mut snapshot = self.snapshot.lock().unwrap();
            snapshot.clear();
            for entry in entries {
                snapshot.insert(entry.id.clone(), entry.clone());
            }
        }

        fn find_entry(&self, id: &str) -> Option<MacWindowEntry> {
            self.snapshot.lock().unwrap().get(id).cloned()
        }

        fn clear_title_cache(&self) {
            // No-op: we no longer cache titles since CG API provides them directly
            // This method is kept for API compatibility
        }
    }

    fn string_for_key(dict: &CFDictionary<CFString, core_foundation::base::CFType>, key: CFStringRef) -> Option<String> {
        let key = unsafe { CFString::wrap_under_get_rule(key) };
        dict.find(&key).and_then(|value| {
            let cf_type = value.clone();
            cf_type
                .downcast::<CFString>()
                .map(|s| s.to_string())
                .filter(|s| !s.trim().is_empty())
        })
    }

    fn number_for_key(
        dict: &CFDictionary<CFString, core_foundation::base::CFType>,
        key: CFStringRef,
    ) -> Option<i64> {
        let key = unsafe { CFString::wrap_under_get_rule(key) };
        dict.find(&key)
            .and_then(|value| value.clone().downcast::<CFNumber>())
            .and_then(|number| number.to_i64())
    }

    fn activate_app(app_name: &str) -> Result<(), String> {
        if app_name.is_empty() {
            return Err("missing app name for activation".into());
        }

        // Prefer LaunchServices activation to avoid per-app automation prompts.
        let open_status = Command::new("open")
            .arg("-a")
            .arg(app_name)
            .status()
            .map_err(|error| format!("activation failed: {error}"))?;

        // Ensure the app is frontmost even if `open` cannot resolve the name; this uses
        // System Events (Accessibility) instead of per-app automation prompts.
        let _ = Command::new("osascript")
            .arg("-e")
            .arg(format!(
                r#"tell application "System Events" to if exists process "{}" then set frontmost of process "{}" to true"#,
                app_name, app_name
            ))
            .status();

        if open_status.success() {
            Ok(())
        } else {
            Err(format!("open -a returned status {open_status:?}"))
        }
    }

    type AXUIElementRef = *const std::ffi::c_void;
    type AXError = i32;
    type CGImageRef = *const std::ffi::c_void;
    type CGWindowID = u32;

    #[allow(non_upper_case_globals)]
    const kAXErrorSuccess: AXError = 0;

    // CGRectNull is used to indicate that the system should determine the bounds automatically
    fn cg_rect_null() -> CGRect {
        CGRect::new(
            &core_graphics::geometry::CGPoint::new(f64::INFINITY, f64::INFINITY),
            &core_graphics::geometry::CGSize::new(0.0, 0.0),
        )
    }

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
        fn AXUIElementCopyAttributeValue(
            element: AXUIElementRef,
            attribute: CFStringRef,
            value: *mut CFTypeRef,
        ) -> AXError;
        fn AXUIElementPerformAction(
            element: AXUIElementRef,
            action: CFStringRef,
        ) -> AXError;
        fn CFRelease(cf: CFTypeRef);
        fn CFArrayGetCount(array: CFTypeRef) -> isize;
        fn CFArrayGetValueAtIndex(array: CFTypeRef, idx: isize) -> *const std::ffi::c_void;
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGPreflightScreenCaptureAccess() -> bool;
        fn CGWindowListCreateImage(
            screen_bounds: CGRect,
            list_option: u32,
            window_id: CGWindowID,
            image_option: u32,
        ) -> CGImageRef;
        fn CGImageGetWidth(image: CGImageRef) -> usize;
        fn CGImageGetHeight(image: CGImageRef) -> usize;
        fn CGImageGetDataProvider(image: CGImageRef) -> *const std::ffi::c_void;
        fn CGDataProviderCopyData(provider: *const std::ffi::c_void) -> CFTypeRef;
        fn CFDataGetBytePtr(data: CFTypeRef) -> *const u8;
        fn CFDataGetLength(data: CFTypeRef) -> isize;
        fn CGImageGetBytesPerRow(image: CGImageRef) -> usize;
        fn CGImageRelease(image: CGImageRef);

        // CGContext functions for hardware-accelerated scaling
        fn CGColorSpaceCreateDeviceRGB() -> *const std::ffi::c_void;
        fn CGColorSpaceRelease(color_space: *const std::ffi::c_void);
        fn CGBitmapContextCreate(
            data: *mut std::ffi::c_void,
            width: usize,
            height: usize,
            bits_per_component: usize,
            bytes_per_row: usize,
            color_space: *const std::ffi::c_void,
            bitmap_info: u32,
        ) -> *const std::ffi::c_void;
        fn CGBitmapContextGetData(context: *const std::ffi::c_void) -> *mut std::ffi::c_void;
        fn CGContextRelease(context: *const std::ffi::c_void);
        fn CGContextDrawImage(context: *const std::ffi::c_void, rect: CGRect, image: CGImageRef);
        fn CGContextSetInterpolationQuality(context: *const std::ffi::c_void, quality: i32);
    }

    // CGBitmapInfo constants
    #[allow(non_upper_case_globals)]
    const kCGImageAlphaPremultipliedLast: u32 = 1;
    #[allow(non_upper_case_globals)]
    const kCGBitmapByteOrder32Big: u32 = 4 << 12;

    // CGInterpolationQuality constants
    #[allow(non_upper_case_globals)]
    const kCGInterpolationHigh: i32 = 3;

    pub fn has_screen_recording_permission() -> bool {
        unsafe { CGPreflightScreenCaptureAccess() }
    }

    #[link(name = "Carbon", kind = "framework")]
    extern "C" {
        fn TISCopyInputSourceForLanguage(language: CFStringRef) -> CFTypeRef;
        fn TISSelectInputSource(input_source: CFTypeRef) -> i32;
    }

    pub fn switch_to_english_input() {
        unsafe {
            let lang = CFString::new("en");
            let source = TISCopyInputSourceForLanguage(lang.as_concrete_TypeRef());
            if !source.is_null() {
                TISSelectInputSource(source);
                CFRelease(source);
            }
        }
    }

    pub fn capture_window_thumbnail(window_id: i64, max_width: u32) -> Option<String> {
        let start = Instant::now();

        unsafe {
            let cg_image = CGWindowListCreateImage(
                cg_rect_null(),
                kCGWindowListOptionIncludingWindow,
                window_id as CGWindowID,
                kCGWindowImageBoundsIgnoreFraming | kCGWindowImageDefault,
            );

            if cg_image.is_null() {
                return None;
            }

            let width = CGImageGetWidth(cg_image);
            let height = CGImageGetHeight(cg_image);

            if width == 0 || height == 0 {
                CGImageRelease(cg_image);
                return None;
            }

            // Calculate target dimensions
            let (new_width, new_height) = if width > max_width as usize {
                let ratio = max_width as f32 / width as f32;
                (max_width as usize, (height as f32 * ratio) as usize)
            } else {
                (width, height)
            };

            // Use CGContext for hardware-accelerated high-quality scaling
            let color_space = CGColorSpaceCreateDeviceRGB();
            let context = CGBitmapContextCreate(
                std::ptr::null_mut(),
                new_width,
                new_height,
                8,
                new_width * 4,
                color_space,
                kCGImageAlphaPremultipliedLast | kCGBitmapByteOrder32Big,
            );
            CGColorSpaceRelease(color_space);

            if context.is_null() {
                CGImageRelease(cg_image);
                return None;
            }

            // Set high quality interpolation
            CGContextSetInterpolationQuality(context, kCGInterpolationHigh);

            // Draw the image scaled to target size
            let rect = CGRect {
                origin: CGPoint { x: 0.0, y: 0.0 },
                size: CGSize { width: new_width as f64, height: new_height as f64 },
            };
            CGContextDrawImage(context, rect, cg_image);
            CGImageRelease(cg_image);

            // Get pixel data directly from context (already in RGBA format)
            let data_ptr = CGBitmapContextGetData(context) as *const u8;
            if data_ptr.is_null() {
                CGContextRelease(context);
                return None;
            }

            // Convert RGBA to RGB for JPEG
            let pixel_count = new_width * new_height;
            let mut rgb_data = Vec::with_capacity(pixel_count * 3);
            for i in 0..pixel_count {
                let offset = i * 4;
                rgb_data.push(*data_ptr.add(offset));     // R
                rgb_data.push(*data_ptr.add(offset + 1)); // G
                rgb_data.push(*data_ptr.add(offset + 2)); // B
            }

            CGContextRelease(context);

            // Encode to JPEG
            let mut jpeg_data = Vec::with_capacity(pixel_count * 3 / 4);
            if image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg_data, 80)
                .write_image(
                    &rgb_data,
                    new_width as u32,
                    new_height as u32,
                    image::ExtendedColorType::Rgb8,
                )
                .is_err()
            {
                return None;
            }

            let base64_str = general_purpose::STANDARD.encode(&jpeg_data);
            let data_url = format!("data:image/jpeg;base64,{}", base64_str);

            let elapsed = start.elapsed().as_millis();
            if elapsed > 50 {
                println!("[thumbnail] window_id={} {}ms", window_id, elapsed);
            }

            Some(data_url)
        }
    }

    fn activate_window_by_title(pid: i32, window_title: &str) -> Result<(), String> {
        unsafe {
            // Create AXUIElement for the application
            let app_ref = AXUIElementCreateApplication(pid);
            if app_ref.is_null() {
                return Err("Failed to create AXUIElement".into());
            }

            // Get the windows array
            let windows_key = CFString::new("AXWindows");
            let mut windows_ref: CFTypeRef = std::ptr::null();

            let err = AXUIElementCopyAttributeValue(
                app_ref,
                windows_key.as_concrete_TypeRef(),
                &mut windows_ref,
            );

            if err != kAXErrorSuccess {
                CFRelease(app_ref as CFTypeRef);
                return Err(format!("Failed to get windows (AX error {})", err));
            }

            if windows_ref.is_null() {
                CFRelease(app_ref as CFTypeRef);
                return Err("Windows array is null".into());
            }

            let window_count = CFArrayGetCount(windows_ref);
            let title_key = CFString::new("AXTitle");
            let raise_action = CFString::new("AXRaise");

            let mut found = false;

            // Iterate through all windows
            for i in 0..window_count {
                let window_ref = CFArrayGetValueAtIndex(windows_ref, i);
                if window_ref.is_null() {
                    continue;
                }

                // Get the window title
                let mut title_ref: CFTypeRef = std::ptr::null();
                let err = AXUIElementCopyAttributeValue(
                    window_ref as AXUIElementRef,
                    title_key.as_concrete_TypeRef(),
                    &mut title_ref,
                );

                if err == kAXErrorSuccess && !title_ref.is_null() {
                    // Convert to Rust string
                    let title_cfstring = CFString::wrap_under_get_rule(title_ref as _);
                    let title = title_cfstring.to_string();

                    // Release the title
                    CFRelease(title_ref);

                    // Check if this is the window we're looking for
                    if title.contains(window_title) {
                        // Perform the raise action
                        let err = AXUIElementPerformAction(
                            window_ref as AXUIElementRef,
                            raise_action.as_concrete_TypeRef(),
                        );

                        if err == kAXErrorSuccess {
                            found = true;
                            break;
                        }
                    }
                }
            }

            // Clean up
            CFRelease(windows_ref);
            CFRelease(app_ref as CFTypeRef);

            if found {
                Ok(())
            } else {
                Err("Window not found or could not be raised".into())
            }
        }
    }

    fn activate_via_pid(pid: i64) -> Result<(), String> {
        unsafe {
            let app = NSRunningApplication::runningApplicationWithProcessIdentifier(nil, pid as i32);
            if app == nil {
                return Err(format!("no running application for pid {pid}"));
            }
            let ok = app.activateWithOptions_(NSApplicationActivateIgnoringOtherApps);
            if ok {
                Ok(())
            } else {
                Err(format!("NSRunningApplication activate failed for pid {pid}"))
            }
        }
    }

    impl WindowProvider for MacWindowProvider {
        fn list(&self, capture_thumbnails: bool) -> Vec<WindowInfo> {
            let started_at = Instant::now();
            let options = kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements;
            let current_pid = std::process::id() as i64;

            let ids_start = Instant::now();
            let Some(window_ids) = create_window_list(options, kCGNullWindowID) else {
                println!(
                    "[rifthold][macos] list_windows failed (window ids); elapsed={}ms",
                    started_at.elapsed().as_millis()
                );
                return Vec::new();
            };
            let ids_elapsed = ids_start.elapsed().as_millis();

            let desc_start = Instant::now();
            let Some(descriptions) = create_description_from_array(window_ids) else {
                println!(
                    "[rifthold][macos] list_windows failed (descriptions); ids_ms={}",
                    ids_elapsed
                );
                return Vec::new();
            };
            let desc_elapsed = desc_start.elapsed().as_millis();

            let iter_start = Instant::now();
            let window_number_key = unsafe { kCGWindowNumber };
            let owner_name_key = unsafe { kCGWindowOwnerName };
            let window_name_key = unsafe { kCGWindowName };
            let owner_pid_key = unsafe { kCGWindowOwnerPID };
            let layer_key = unsafe { kCGWindowLayer };

            let mut fallback_count = 0;
            let mut skipped_layers = 0;
            let mut skipped_self = 0;
            let mut skipped_control_center = 0;

            // First pass: collect all window info and identify apps needing title fetch
            let mut pending_entries = Vec::new();
            for dict in descriptions.iter() {
                let Some(window_number) = number_for_key(&dict, window_number_key) else {
                    continue;
                };

                let id = window_number.to_string();
                let app_name =
                    string_for_key(&dict, owner_name_key).unwrap_or_else(|| "App".into());
                let cg_title = string_for_key(&dict, window_name_key);
                let owner_pid = number_for_key(&dict, owner_pid_key);
                let layer = number_for_key(&dict, layer_key).unwrap_or(0);

                if owner_pid == Some(current_pid) {
                    skipped_self += 1;
                    continue;
                }

                if layer != 0 {
                    skipped_layers += 1;
                    continue;
                }

                if app_name == "Control Center" {
                    skipped_control_center += 1;
                    continue;
                }

                pending_entries.push((id, app_name, cg_title, owner_pid));
            }

            // Second pass: build window entries with CG titles
            let mut entries = Vec::new();

            for (id, app_name, cg_title, owner_pid) in pending_entries {
                // Use CG title if available (requires Screen Recording permission)
                // Otherwise fall back to app name
                let (title, is_fallback) = if let Some(t) = cg_title.filter(|t| !t.trim().is_empty()) {
                    (t, false)
                } else {
                    fallback_count += 1;
                    (app_name.clone(), true)
                };

                entries.push(MacWindowEntry {
                    id,
                    title,
                    app_name,
                    is_title_fallback: is_fallback,
                    owner_pid,
                });
            }

            // Keep the snapshot to resolve activation requests.
            self.refresh_snapshot(&entries);

            let iter_elapsed = iter_start.elapsed().as_millis();
            let elapsed = started_at.elapsed().as_millis();
            println!(
                "[rifthold][macos] list_windows total={} fallback_titles={} skipped_layers={} skipped_self={} skipped_control_center={} ids_ms={} desc_ms={} iter_ms={} total_ms={}",
                entries.len(),
                fallback_count,
                skipped_layers,
                skipped_self,
                skipped_control_center,
                ids_elapsed,
                desc_elapsed,
                iter_elapsed,
                elapsed,
            );

            // Third pass: capture thumbnails (if enabled)
            let results: Vec<WindowInfo> = if capture_thumbnails {
                let thumbnail_start = Instant::now();
                let max_thumbnail_width = 500; // Max width for thumbnail (increased for better quality)

                // Use parallel iterator for faster thumbnail capture
                let results: Vec<WindowInfo> = entries
                    .par_iter()
                    .map(|entry| {
                        let window_id = entry.id.parse::<i64>().unwrap_or(0);
                        let thumbnail = capture_window_thumbnail(window_id, max_thumbnail_width);

                        WindowInfo {
                            id: entry.id.clone(),
                            title: entry.title.clone(),
                            app_name: entry.app_name.clone(),
                            is_title_fallback: entry.is_title_fallback,
                            thumbnail,
                        }
                    })
                    .collect();

                let thumbnail_elapsed = thumbnail_start.elapsed().as_millis();
                let total_elapsed = started_at.elapsed().as_millis();

                println!(
                    "[rifthold][macos] list_windows completed: windows={} thumbnails_captured={} thumbnail_ms={} total_ms={}",
                    results.len(),
                    results.iter().filter(|w| w.thumbnail.is_some()).count(),
                    thumbnail_elapsed,
                    total_elapsed
                );

                results
            } else {
                // No thumbnails
                let results: Vec<WindowInfo> = entries
                    .into_iter()
                    .map(|entry| WindowInfo {
                        id: entry.id,
                        title: entry.title,
                        app_name: entry.app_name,
                        is_title_fallback: entry.is_title_fallback,
                        thumbnail: None,
                    })
                    .collect();

                results
            };

            results
        }

        fn activate(&self, id: &str) -> Result<(), String> {
            // Try the cached snapshot, then refresh once if missing.
            let entry = self.find_entry(id).or_else(|| {
                let _ = self.list(false); // Don't need thumbnails for activation
                self.find_entry(id)
            });

            let Some(entry) = entry else {
                return Err(format!("window id {id} not found"));
            };

            // First, activate the application to bring it to the foreground
            let app_activated = if let Some(pid) = entry.owner_pid {
                activate_via_pid(pid).is_ok()
            } else {
                false
            };

            if !app_activated {
                activate_app(&entry.app_name)?;
            }

            // Then, activate the specific window by title using Accessibility API
            // Only try this if we have a real title (not a fallback) and a PID
            if !entry.is_title_fallback {
                if let Some(pid) = entry.owner_pid {
                    // Give the app a moment to become active
                    std::thread::sleep(std::time::Duration::from_millis(150));

                    if let Err(error) = activate_window_by_title(pid as i32, &entry.title) {
                        eprintln!("[rifthold] activate_window_by_title failed: {error}");
                    }
                }
            }

            Ok(())
        }

        fn clear_cache(&self) {
            self.clear_title_cache()
        }
    }
}
