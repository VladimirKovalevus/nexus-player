use image::imageops::FilterType;

use crate::error::StreamingError;

/// Decode arbitrary cover art, center-crop it to a square and re-encode it as WebP.
///
/// The crop matches the `object-fit: cover` the UI already applies to every
/// thumbnail, so cropping here costs nothing visually and saves the pixels.
///
/// Never upscales: providers are free to serve something smaller than `target`
/// (SoundCloud's `-large.jpg` is 100x100), and blowing that up would only waste
/// bytes to look blurrier. The encoded edge is `min(target, shorter source edge)`.
pub(crate) fn to_square_webp(
    bytes: &[u8],
    target: u32,
    quality: f32,
) -> Result<Vec<u8>, StreamingError> {
    let img =
        image::load_from_memory(bytes).map_err(|e| StreamingError::ImageDecode(e.to_string()))?;

    let (width, height) = (img.width(), img.height());
    if width == 0 || height == 0 {
        return Err(StreamingError::ImageDecode(format!(
            "zero-sized image ({width}x{height})"
        )));
    }

    let side = width.min(height);
    let edge = target.min(side);

    let cropped = img.crop_imm((width - side) / 2, (height - side) / 2, side, side);
    let scaled = if side == edge {
        cropped
    } else {
        cropped.resize_exact(edge, edge, FilterType::Lanczos3)
    };

    let rgba = scaled.to_rgba8();
    let encoded = webp::Encoder::from_rgba(rgba.as_raw(), edge, edge).encode(quality);

    Ok(encoded.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, RgbImage};

    fn source(width: u32, height: u32) -> Vec<u8> {
        let mut img = RgbImage::new(width, height);
        // A gradient, so resampling has something real to chew on.
        for (x, y, px) in img.enumerate_pixels_mut() {
            *px = image::Rgb([(x % 256) as u8, (y % 256) as u8, 128]);
        }
        let mut out = std::io::Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(img)
            .write_to(&mut out, image::ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    fn dimensions(webp_bytes: &[u8]) -> (u32, u32) {
        let decoded = image::load_from_memory(webp_bytes).expect("re-decode webp");
        (decoded.width(), decoded.height())
    }

    #[test]
    fn downscales_to_target() {
        let out = to_square_webp(&source(1200, 1200), 256, 80.0).unwrap();
        assert_eq!(dimensions(&out), (256, 256));
    }

    #[test]
    fn never_upscales_a_small_source() {
        // SoundCloud's -large.jpg shape: smaller than the target on both edges.
        let out = to_square_webp(&source(100, 100), 256, 80.0).unwrap();
        assert_eq!(dimensions(&out), (100, 100));
    }

    #[test]
    fn crops_non_square_to_square() {
        let out = to_square_webp(&source(800, 400), 256, 80.0).unwrap();
        assert_eq!(dimensions(&out), (256, 256));

        // Shorter edge is 400, so a 512 target must clamp to 400 rather than upscale.
        let out = to_square_webp(&source(800, 400), 512, 80.0).unwrap();
        assert_eq!(dimensions(&out), (400, 400));
    }

    #[test]
    fn output_is_smaller_than_a_full_size_source() {
        let raw = source(1200, 1200);
        let out = to_square_webp(&raw, 256, 80.0).unwrap();
        assert!(
            out.len() < raw.len(),
            "expected a saving, got {} -> {}",
            raw.len(),
            out.len()
        );
    }

    #[test]
    fn rejects_garbage() {
        let err = to_square_webp(b"definitely not an image", 256, 80.0).unwrap_err();
        assert!(matches!(err, StreamingError::ImageDecode(_)));
    }
}
