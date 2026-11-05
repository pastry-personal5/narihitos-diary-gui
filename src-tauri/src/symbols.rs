//! `get_symbol`: allowlisted SF Symbol lookup rendered to a bounded template PNG.
use serde::Serialize;

pub const NAMES: [&str; 6] = [
    "doc.badge.plus",
    "command",
    "gearshape",
    "sparkles",
    "xmark",
    "arrow.clockwise",
];
pub const SIZES: [u16; 4] = [12, 14, 16, 20];
pub const SCALES: [u8; 2] = [1, 2];
pub const MAX_PNG: usize = 32 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum Invalid {
    Name,
    Size,
    Scale,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Png(Vec<u8>),
    Unsupported,
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SymbolResult {
    Ok { png_base64: String },
    Unsupported,
}

impl From<Outcome> for SymbolResult {
    fn from(outcome: Outcome) -> Self {
        match outcome {
            Outcome::Png(bytes) => Self::Ok {
                png_base64: base64(&bytes),
            },
            Outcome::Unsupported => Self::Unsupported,
        }
    }
}

/// Rejects anything outside the allowlist before any AppKit call.
pub fn validate(name: &str, size: u16, scale: u8) -> Result<(), Invalid> {
    if !NAMES.contains(&name) {
        Err(Invalid::Name)
    } else if !SIZES.contains(&size) {
        Err(Invalid::Size)
    } else if !SCALES.contains(&scale) {
        Err(Invalid::Scale)
    } else {
        Ok(())
    }
}

fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | u32::from(*b) << (16 - 8 * i));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Must run on the main thread. Draws the symbol as a black template so the client can tint it.
#[cfg(target_os = "macos")]
pub fn render(name: &str, size: u16, scale: u8) -> Outcome {
    use objc2::AnyThread;
    use objc2_app_kit::{
        NSBitmapFormat, NSBitmapImageFileType, NSBitmapImageRep, NSCompositingOperation,
        NSDeviceRGBColorSpace, NSGraphicsContext, NSImage,
    };
    use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize, NSString};
    let Some(image) = NSImage::imageWithSystemSymbolName_accessibilityDescription(
        &NSString::from_str(name),
        None,
    ) else {
        return Outcome::Unsupported;
    };
    let px = size as isize * scale as isize;
    // SAFETY: null planes ask AppKit to allocate; the other arguments describe a plain RGBA bitmap.
    let rep = unsafe {
        NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bitmapFormat_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            std::ptr::null_mut(),
            px,
            px,
            8,
            4,
            true,
            false,
            NSDeviceRGBColorSpace,
            NSBitmapFormat(0),
            0,
            0,
        )
    };
    let Some(rep) = rep else {
        return Outcome::Unsupported;
    };
    let side = f64::from(size);
    rep.setSize(NSSize::new(side, side));
    let Some(context) = NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep) else {
        return Outcome::Unsupported;
    };
    let natural = image.size();
    let fit = side / natural.width.max(natural.height).max(1.0);
    let (w, h) = (natural.width * fit, natural.height * fit);
    NSGraphicsContext::saveGraphicsState_class();
    NSGraphicsContext::setCurrentContext(Some(&context));
    image.drawInRect_fromRect_operation_fraction(
        NSRect::new(
            NSPoint::new((side - w) / 2.0, (side - h) / 2.0),
            NSSize::new(w, h),
        ),
        NSRect::ZERO,
        NSCompositingOperation::SourceOver,
        1.0,
    );
    NSGraphicsContext::restoreGraphicsState_class();
    // A context that did not take effect still encodes a valid, fully transparent PNG.
    let (len, data) = (rep.bytesPerRow() as usize * px as usize, rep.bitmapData());
    // SAFETY: `bitmapData` points at `bytesPerRow * pixelsHigh` bytes owned by `rep`, alive here.
    let blank = data.is_null()
        || unsafe { std::slice::from_raw_parts(data, len) }
            .chunks(4)
            .all(|pixel| pixel.get(3) == Some(&0));
    if blank {
        return Outcome::Unsupported;
    }
    // SAFETY: an empty property dictionary is valid for PNG output.
    let data = unsafe {
        rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    };
    match data.map(|d| d.to_vec()) {
        Some(bytes) if !bytes.is_empty() && bytes.len() <= MAX_PNG => Outcome::Png(bytes),
        _ => Outcome::Unsupported,
    }
}

#[cfg(not(target_os = "macos"))]
pub fn render(_name: &str, _size: u16, _scale: u8) -> Outcome {
    Outcome::Unsupported
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unlisted_name() {
        assert_eq!(validate("trash", 16, 2), Err(Invalid::Name));
        assert_eq!(validate("", 16, 2), Err(Invalid::Name));
    }

    #[test]
    fn rejects_unlisted_size_or_scale() {
        assert_eq!(validate("xmark", 4096, 1), Err(Invalid::Size));
        assert_eq!(validate("xmark", 16, 0), Err(Invalid::Scale));
        assert_eq!(validate("xmark", 16, 3), Err(Invalid::Scale));
        assert_eq!(validate("xmark", 16, 2), Ok(()));
    }

    #[test]
    fn base64_matches_reference() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn result_serializes_status_tag() {
        let ok = serde_json::to_string(&SymbolResult::from(Outcome::Png(vec![1]))).unwrap();
        assert_eq!(ok, r#"{"status":"ok","png_base64":"AQ=="}"#);
        let no = serde_json::to_string(&SymbolResult::from(Outcome::Unsupported)).unwrap();
        assert_eq!(no, r#"{"status":"unsupported"}"#);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn png_has_signature_and_cap() {
        // Symbols may be unavailable on the host; when present they must be a bounded PNG.
        for name in NAMES {
            if let Outcome::Png(bytes) = render(name, 16, 2) {
                assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
                assert!(bytes.len() <= MAX_PNG);
            }
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn unknown_symbol_is_unsupported() {
        assert_eq!(render("no.such.symbol.exists", 16, 1), Outcome::Unsupported);
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn non_macos_is_unsupported() {
        assert_eq!(render("xmark", 16, 1), Outcome::Unsupported);
    }
}
