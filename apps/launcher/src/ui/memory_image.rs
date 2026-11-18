//! Shared RGBA-to-GPUI conversion. No file paths, temporary files or persistent cache.
use gpui_kit::RenderImage;
use std::sync::Arc;

pub(crate) fn rgba(width: u32, height: u32, bytes: &[u8], opaque: bool) -> Option<Arc<RenderImage>> {
    let size = (width as usize).checked_mul(height as usize)?.checked_mul(4)?;
    if width == 0 || height == 0 || bytes.len() != size {
        return None;
    }
    let mut bgra = bytes.to_vec();
    for pixel in bgra.chunks_exact_mut(4) {
        pixel.swap(0, 2);
        if opaque {
            pixel[3] = 255;
        }
    }
    let buffer = image::RgbaImage::from_raw(width, height, bgra)?;
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpui_pixels_use_bgra_and_preserve_badge_alpha() {
        let pixels = [10, 20, 30, 0, 40, 50, 60, 128];
        let image = rgba(2, 1, &pixels, false).unwrap();
        assert_eq!(image.as_bytes(0).unwrap(), &[30, 20, 10, 0, 60, 50, 40, 128]);
        assert_eq!(image.frame_count(), 1);
    }

    #[test]
    fn preview_frames_keep_their_previous_opaque_alpha() {
        let image = rgba(1, 1, &[10, 20, 30, 0], true).unwrap();
        assert_eq!(image.as_bytes(0).unwrap(), &[30, 20, 10, 255]);
        assert!(rgba(0, 1, &[], false).is_none());
        assert!(rgba(1, 1, &[1, 2, 3], false).is_none());
        assert!(rgba(u32::MAX, u32::MAX, &[], false).is_none());
    }
}
