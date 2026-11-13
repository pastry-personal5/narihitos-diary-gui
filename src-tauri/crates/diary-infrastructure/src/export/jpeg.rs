//! System ImageIO JPEG preparation. No local file is changed or external tool invoked.
use diary_application::export::ExportError;
use std::{ffi::c_void, ptr};
type Ref = *const c_void;
#[repr(C)]
struct Rect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFDataCreate(a: Ref, bytes: *const u8, len: isize) -> Ref;
    fn CFDataCreateMutable(a: Ref, capacity: isize) -> Ref;
    fn CFDataGetLength(data: Ref) -> isize;
    fn CFDataGetBytePtr(data: Ref) -> *const u8;
    fn CFStringCreateWithCString(a: Ref, text: *const i8, encoding: u32) -> Ref;
    fn CFNumberCreate(a: Ref, kind: i32, value: Ref) -> Ref;
    fn CFDictionaryCreate(
        a: Ref,
        keys: *const Ref,
        values: *const Ref,
        count: isize,
        kc: Ref,
        vc: Ref,
    ) -> Ref;
    fn CFDictionaryGetValue(dictionary: Ref, key: Ref) -> Ref;
    fn CFNumberGetValue(number: Ref, kind: i32, value: *mut c_void) -> bool;
    fn CFRelease(value: Ref);
}
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    static kCGColorSpaceSRGB: Ref;
    fn CGColorSpaceCreateWithName(name: Ref) -> Ref;
    fn CGColorSpaceRelease(value: Ref);
    fn CGBitmapContextCreate(
        data: *mut c_void,
        width: usize,
        height: usize,
        bits: usize,
        stride: usize,
        space: Ref,
        info: u32,
    ) -> Ref;
    fn CGContextSetRGBFillColor(context: Ref, r: f64, g: f64, b: f64, a: f64);
    fn CGContextFillRect(context: Ref, rect: Rect);
    fn CGContextDrawImage(context: Ref, rect: Rect, image: Ref);
    fn CGContextSetInterpolationQuality(context: Ref, quality: i32);
    fn CGBitmapContextCreateImage(context: Ref) -> Ref;
    fn CGContextRelease(value: Ref);
    fn CGImageRelease(value: Ref);
}
#[link(name = "ImageIO", kind = "framework")]
unsafe extern "C" {
    static kCGImageDestinationLossyCompressionQuality: Ref;
    static kCGImagePropertyOrientation: Ref;
    fn CGImageSourceCopyPropertiesAtIndex(source: Ref, index: usize, options: Ref) -> Ref;
    fn CGImageSourceCreateWithData(data: Ref, options: Ref) -> Ref;
    fn CGImageSourceCreateImageAtIndex(source: Ref, index: usize, options: Ref) -> Ref;
    fn CGImageDestinationCreateWithData(data: Ref, kind: Ref, count: usize, options: Ref) -> Ref;
    fn CGImageDestinationAddImage(destination: Ref, image: Ref, properties: Ref);
    fn CGImageDestinationFinalize(destination: Ref) -> bool;
}
struct Owned(Ref, unsafe extern "C" fn(Ref));
impl Owned {
    fn new(value: Ref, release: unsafe extern "C" fn(Ref)) -> Result<Self, ExportError> {
        if value.is_null() {
            Err(ExportError::Storage)
        } else {
            Ok(Self(value, release))
        }
    }
}
impl Drop for Owned {
    fn drop(&mut self) {
        unsafe { (self.1)(self.0) }
    }
}

pub(super) fn convert(png: &[u8]) -> Result<Vec<u8>, ExportError> {
    let (_, w, h) = crate::image::validate_bytes(png).map_err(|_| ExportError::Invalid)?;
    let width = ((u64::from(w) * 480 + u64::from(h) / 2) / u64::from(h)).max(1) as usize;
    if width * 480 > 16_000_000 {
        return Err(ExportError::Limit);
    }
    // SAFETY: every created object is checked and owned by an RAII release guard. Data and
    // dictionary values remain alive throughout ImageIO encoding. Geometry/allocation is bounded.
    unsafe {
        let data = Owned::new(
            CFDataCreate(ptr::null(), png.as_ptr(), png.len() as isize),
            CFRelease,
        )?;
        let source = Owned::new(CGImageSourceCreateWithData(data.0, ptr::null()), CFRelease)?;
        // Generated PNGs can carry EXIF prompts/settings. Only a non-default orientation
        // prevents this raw-pixel conversion; metadata itself is not invalid input.
        let properties = Owned::new(
            CGImageSourceCopyPropertiesAtIndex(source.0, 0, ptr::null()),
            CFRelease,
        )?;
        let orientation = CFDictionaryGetValue(properties.0, kCGImagePropertyOrientation);
        if !orientation.is_null() {
            let mut value = 0i32;
            if !CFNumberGetValue(orientation, 3, (&mut value as *mut i32).cast()) || value != 1 {
                return Err(ExportError::Invalid);
            }
        }

        let image = Owned::new(
            CGImageSourceCreateImageAtIndex(source.0, 0, ptr::null()),
            CGImageRelease,
        )?;
        let space = Owned::new(
            CGColorSpaceCreateWithName(kCGColorSpaceSRGB),
            CGColorSpaceRelease,
        )?;
        let context = Owned::new(
            CGBitmapContextCreate(ptr::null_mut(), width, 480, 8, width * 4, space.0, 5),
            CGContextRelease,
        )?;
        CGContextSetRGBFillColor(context.0, 1.0, 1.0, 1.0, 1.0);
        CGContextFillRect(
            context.0,
            Rect {
                x: 0.0,
                y: 0.0,
                width: width as f64,
                height: 480.0,
            },
        );
        CGContextSetInterpolationQuality(context.0, 3);
        CGContextDrawImage(
            context.0,
            Rect {
                x: 0.0,
                y: 0.0,
                width: width as f64,
                height: 480.0,
            },
            image.0,
        );
        let resized = Owned::new(CGBitmapContextCreateImage(context.0), CGImageRelease)?;
        let output = Owned::new(CFDataCreateMutable(ptr::null(), 0), CFRelease)?;
        let kind = Owned::new(
            CFStringCreateWithCString(ptr::null(), c"public.jpeg".as_ptr(), 0x08000100),
            CFRelease,
        )?;
        let encoder = Owned::new(
            CGImageDestinationCreateWithData(output.0, kind.0, 1, ptr::null()),
            CFRelease,
        )?;
        let quality = 0.85f64;
        let number = Owned::new(
            CFNumberCreate(ptr::null(), 13, (&quality as *const f64).cast()),
            CFRelease,
        )?;
        let options = Owned::new(
            CFDictionaryCreate(
                ptr::null(),
                &kCGImageDestinationLossyCompressionQuality,
                &number.0,
                1,
                ptr::null(),
                ptr::null(),
            ),
            CFRelease,
        )?;
        CGImageDestinationAddImage(encoder.0, resized.0, options.0);
        if !CGImageDestinationFinalize(encoder.0) {
            return Err(ExportError::Storage);
        }
        let size = CFDataGetLength(output.0);
        if size <= 0 || size > 5 * 1024 * 1024 {
            return Err(ExportError::Limit);
        }
        Ok(std::slice::from_raw_parts(CFDataGetBytePtr(output.0), size as usize).to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGImageGetWidth(image: Ref) -> usize;
        fn CGImageGetHeight(image: Ref) -> usize;
        fn CGImageGetDataProvider(image: Ref) -> Ref;
        fn CGDataProviderCopyData(provider: Ref) -> Ref;
    }
    #[test]
    fn generation_exif_is_accepted_without_copying_metadata_or_rotating_pixels() {
        let plain = include_bytes!("../../tests/fixtures/illustrated/transparent-wide.png");
        let expected = convert(plain).unwrap();
        for png in [
            include_bytes!("../../tests/fixtures/illustrated/generation-exif.png").as_slice(),
            include_bytes!("../../tests/fixtures/illustrated/orientation-1.png").as_slice(),
        ] {
            let original = png.to_vec();
            assert_eq!(convert(png).unwrap(), expected);
            assert_eq!(png, original);
        }
        assert_eq!(
            convert(include_bytes!(
                "../../tests/fixtures/illustrated/orientation-6.png"
            )),
            Err(ExportError::Invalid)
        );
    }

    #[test]
    fn jpeg_is_height_480_preserves_aspect_flattens_white_and_leaves_input_intact() {
        for (png, width) in [
            (
                include_bytes!("../../tests/fixtures/illustrated/transparent-wide.png").as_slice(),
                960,
            ),
            (
                include_bytes!("../../tests/fixtures/illustrated/transparent-tall.png").as_slice(),
                240,
            ),
        ] {
            let original = png.to_vec();
            let jpeg = convert(png).unwrap();
            assert_eq!(png, original);
            assert!(jpeg.starts_with(&[0xff, 0xd8]));
            assert!(jpeg.ends_with(&[0xff, 0xd9]));
            unsafe {
                let data = Owned::new(
                    CFDataCreate(ptr::null(), jpeg.as_ptr(), jpeg.len() as isize),
                    CFRelease,
                )
                .unwrap();
                let source =
                    Owned::new(CGImageSourceCreateWithData(data.0, ptr::null()), CFRelease)
                        .unwrap();
                let image = Owned::new(
                    CGImageSourceCreateImageAtIndex(source.0, 0, ptr::null()),
                    CGImageRelease,
                )
                .unwrap();
                assert_eq!(CGImageGetWidth(image.0), width);
                assert_eq!(CGImageGetHeight(image.0), 480);
                let pixels = Owned::new(
                    CGDataProviderCopyData(CGImageGetDataProvider(image.0)),
                    CFRelease,
                )
                .unwrap();
                let first_pixel = std::slice::from_raw_parts(CFDataGetBytePtr(pixels.0), 3);
                assert!(
                    first_pixel.iter().all(|c| *c > 245),
                    "transparent red must become white"
                );
            }
        }
        assert_eq!(convert(b"not a PNG"), Err(ExportError::Invalid));
        assert_eq!(
            convert(include_bytes!(
                "../../tests/fixtures/illustrated/excessive-resize.png"
            )),
            Err(ExportError::Limit)
        );
    }
}
