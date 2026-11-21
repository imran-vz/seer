//! The parts of macOS that GPUI does not reach: system colours, Finder's
//! file icons, the app's appearance, the traffic lights and ejecting a
//! disk. Everything here except [`eject`] must be called on the main
//! thread. Views do not call this directly for
//! anything that differs per OS; that goes through `platform`.

use block2::RcBlock;
use gpui_kit::{Hsla, RenderImage, Rgba};
use image::{Frame, RgbaImage};
use objc2_app_kit::{
    NSAppearance, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSColor, NSColorSpace,
    NSCompositingOperation, NSGraphicsContext, NSWorkspace,
};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_core_graphics::{
    CGBitmapContextCreate, CGBitmapContextGetData, CGColorSpace, CGImageAlphaInfo,
    CGImageByteOrderInfo,
};
use objc2_foundation::NSString;
use smallvec::SmallVec;
use std::cell::Cell;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

pub struct SystemColors {
    /// The user's accent colour.
    pub accent: Hsla,
    /// Background of selected rows in a focused list.
    pub selection: Hsla,
}

/// Reads the user's accent and selection colours for the given appearance.
pub fn system_colors(dark: bool) -> SystemColors {
    let fallback = SystemColors {
        accent: gpui_kit::rgb(0x0A84FF).into(),
        selection: gpui_kit::rgb(0x0059D1).into(),
    };
    let out = Rc::new(Cell::new(None));
    let sink = out.clone();
    // Dynamic colours resolve against the current drawing appearance.
    let read = RcBlock::new(move || {
        let accent = srgb(&NSColor::controlAccentColor());
        let selection = srgb(&NSColor::selectedContentBackgroundColor());
        sink.set(accent.zip(selection));
    });
    unsafe {
        let name = if dark {
            NSAppearanceNameDarkAqua
        } else {
            NSAppearanceNameAqua
        };
        if let Some(appearance) = NSAppearance::appearanceNamed(name) {
            appearance.performAsCurrentDrawingAppearance(&read);
        }
    }
    match out.get() {
        Some((accent, selection)) => SystemColors { accent, selection },
        None => fallback,
    }
}

fn srgb(color: &NSColor) -> Option<Hsla> {
    let color = color.colorUsingColorSpace(&NSColorSpace::sRGBColorSpace())?;
    Some(
        Rgba {
            r: color.redComponent() as f32,
            g: color.greenComponent() as f32,
            b: color.blueComponent() as f32,
            a: color.alphaComponent() as f32,
        }
        .into(),
    )
}

/// Finder's icon for a file or folder, rendered at `pixels` square.
pub fn file_icon(path: &Path, is_dir: bool, pixels: usize) -> Option<Arc<RenderImage>> {
    let workspace = NSWorkspace::sharedWorkspace();
    let icon = if seer_core::mtp::is_device_path(path) {
        // Nothing on disk to ask about, so go by the type alone. "'fldr'"
        // is the system's code for a plain folder.
        let kind = match (is_dir, path.extension()) {
            (true, _) => "'fldr'".to_owned(),
            (false, Some(extension)) => extension.to_string_lossy().into_owned(),
            (false, None) => String::new(),
        };
        #[allow(deprecated)]
        workspace.iconForFileType(&NSString::from_str(&kind))
    } else {
        workspace.iconForFile(&NSString::from_str(&path.to_string_lossy()))
    };

    let space = CGColorSpace::new_device_rgb()?;
    // BGRA in memory, which is what GPUI's renderer takes.
    let layout = CGImageAlphaInfo::PremultipliedFirst.0 | CGImageByteOrderInfo::Order32Little.0;
    let mut bgra = unsafe {
        let context = CGBitmapContextCreate(
            std::ptr::null_mut(),
            pixels,
            pixels,
            8,
            pixels * 4,
            Some(&space),
            layout,
        )?;
        let graphics = NSGraphicsContext::graphicsContextWithCGContext_flipped(&context, false);
        NSGraphicsContext::saveGraphicsState_class();
        NSGraphicsContext::setCurrentContext(Some(&graphics));
        icon.drawInRect_fromRect_operation_fraction(
            CGRect::new(
                CGPoint::new(0., 0.),
                CGSize::new(pixels as f64, pixels as f64),
            ),
            CGRect::ZERO,
            NSCompositingOperation::SourceOver,
            1.0,
        );
        NSGraphicsContext::restoreGraphicsState_class();

        let data = CGBitmapContextGetData(Some(&context)) as *const u8;
        if data.is_null() {
            return None;
        }
        std::slice::from_raw_parts(data, pixels * pixels * 4).to_vec()
    };

    // Core Graphics premultiplies alpha; GPUI expects straight alpha.
    for px in bgra.as_chunks_mut::<4>().0 {
        let a = px[3] as u32;
        if a != 0 && a != 255 {
            for c in &mut px[..3] {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }

    let buffer = RgbaImage::from_raw(pixels as u32, pixels as u32, bgra)?;
    Some(Arc::new(RenderImage::new(SmallVec::from_elem(
        Frame::new(buffer),
        1,
    ))))
}

/// Icons by file type and size in points.
type IconCache = std::collections::HashMap<(String, usize), Option<Arc<RenderImage>>>;

thread_local! {
    static ICONS: std::cell::RefCell<IconCache> = Default::default();
}

/// Finder icon for an entry, cached per file type. `points` is the display
/// size; the bitmap is rendered at twice that for Retina.
pub fn icon_for(path: &Path, is_dir: bool, points: usize) -> Option<Arc<RenderImage>> {
    let key = if is_dir {
        "/".to_owned()
    } else {
        path.extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default()
    };
    ICONS.with_borrow_mut(|icons| {
        icons
            .entry((key, points))
            .or_insert_with(|| file_icon(path, is_dir, points * 2))
            .clone()
    })
}

/// Forces the app to light or dark, or follows the system when `None`.
pub fn set_appearance(dark: Option<bool>) {
    let Some(mtm) = objc2::MainThreadMarker::new() else {
        return;
    };
    let appearance = dark.and_then(|dark| unsafe {
        NSAppearance::appearanceNamed(if dark {
            NSAppearanceNameDarkAqua
        } else {
            NSAppearanceNameAqua
        })
    });
    objc2_app_kit::NSApplication::sharedApplication(mtm).setAppearance(appearance.as_deref());
}

/// Hides the traffic lights of every window, so `ui::window_controls` can
/// stand in for another OS's controls (`SEER_PLATFORM`). Best effort: GPUI
/// may show them again if it re-creates the titlebar.
pub fn hide_window_buttons() {
    use objc2_app_kit::NSWindowButton;
    let Some(mtm) = objc2::MainThreadMarker::new() else {
        return;
    };
    for window in objc2_app_kit::NSApplication::sharedApplication(mtm).windows() {
        for kind in [
            NSWindowButton::CloseButton,
            NSWindowButton::MiniaturizeButton,
            NSWindowButton::ZoomButton,
        ] {
            if let Some(button) = window.standardWindowButton(kind) {
                button.setHidden(true);
            }
        }
    }
}

/// Unmounts and ejects the disk mounted at `path`. Blocks until the system
/// is done, so call it off the main thread (the one exception to this
/// module's main-thread rule: NSWorkspace may be used from any thread).
/// False when the disk is busy or refuses.
pub fn eject(path: &Path) -> bool {
    use objc2_foundation::NSURL;
    let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
    NSWorkspace::sharedWorkspace()
        .unmountAndEjectDeviceAtURL_error(&url)
        .is_ok()
}
