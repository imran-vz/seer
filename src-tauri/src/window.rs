//! Window configuration and platform-specific customization
//!
//! This module handles window setup and platform-specific styling,
//! particularly for macOS transparent titlebar and background color.

use tauri::{LogicalPosition, TitleBarStyle, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

/// Background color for the window (#131313)
const BG_COLOR_COMPONENT: f64 = 0.0745; // 19/255

/// Create and configure the main application window
pub fn create_main_window(app: &tauri::App) -> Result<WebviewWindow, tauri::Error> {
    let win_builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
        .title("Seer")
        .inner_size(1200.0, 800.0)
        .min_inner_size(800.0, 600.0);

    #[cfg(target_os = "macos")]
    let win_builder = win_builder
        .title_bar_style(TitleBarStyle::Overlay)
        .hidden_title(true)
        .traffic_light_position(LogicalPosition::new(16.0, 24.0));

    let window = win_builder.build()?;

    #[cfg(target_os = "macos")]
    apply_macos_styling(&window);

    Ok(window)
}

/// Apply macOS-specific window styling
///
/// This configures:
/// - Transparent titlebar
/// - Custom background color matching the app theme
#[cfg(target_os = "macos")]
fn apply_macos_styling(window: &WebviewWindow) {
    use objc2::rc::Retained;
    use objc2_app_kit::{NSColor, NSWindow};
    use objc2_foundation::MainThreadMarker;

    let ns_window: Retained<NSWindow> = unsafe {
        let ptr: *mut std::ffi::c_void = window.ns_window().unwrap();
        Retained::retain(ptr as *mut NSWindow).unwrap()
    };

    // Verify we're on the main thread (required for AppKit operations)
    let _mtm = MainThreadMarker::new().expect("must be on the main thread");

    let bg_color = NSColor::colorWithSRGBRed_green_blue_alpha(
        BG_COLOR_COMPONENT,
        BG_COLOR_COMPONENT,
        BG_COLOR_COMPONENT,
        1.0,
    );

    ns_window.setTitlebarAppearsTransparent(true);
    ns_window.setBackgroundColor(Some(&bg_color));
}
