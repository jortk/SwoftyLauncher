//! Small helper-supplied PNGs. Bound decoding and cache successful and failed inputs.
use std::collections::VecDeque;
use std::io::Cursor;
use std::sync::Arc;

use base64::Engine;
use gpui_kit::RenderImage;

const ENCODED_LIMIT: usize = 64 * 1024;
const SIDE_LIMIT: u32 = 256;
const CACHE_LIMIT: usize = 64;

#[derive(Default)]
pub(super) struct Images {
    entries: VecDeque<(String, Option<Arc<RenderImage>>)>,
    retired: Vec<Arc<RenderImage>>,
}

impl Images {
    pub(super) fn get(&mut self, data: &str) -> Option<Arc<RenderImage>> {
        if data.len() > ENCODED_LIMIT {
            return None;
        }
        if let Some(at) = self.entries.iter().position(|(key, _)| key == data) {
            let entry = self.entries.remove(at)?;
            let image = entry.1.clone();
            self.entries.push_back(entry);
            return image;
        }
        let image = decode(data).and_then(|(w, h, rgba)| crate::ui::memory_image::rgba(w, h, &rgba, false));
        if self.entries.len() == CACHE_LIMIT
            && let Some((_, Some(image))) = self.entries.pop_front()
        {
            self.retired.push(image);
        }
        self.entries.push_back((data.into(), image.clone()));
        image
    }

    pub(super) fn retired(&mut self) -> Vec<Arc<RenderImage>> {
        std::mem::take(&mut self.retired)
    }

    pub(super) fn clear(&mut self) -> Vec<Arc<RenderImage>> {
        let mut out = self.retired();
        out.extend(self.entries.drain(..).filter_map(|(_, image)| image));
        out
    }
}

/// Decode straight-alpha RGBA before handing pixels to the shared memory renderer.
fn decode(data: &str) -> Option<(u32, u32, Vec<u8>)> {
    if data.len() > ENCODED_LIMIT {
        return None;
    }
    let bytes = base64::engine::general_purpose::STANDARD.decode(data).ok()?;
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_limits(png::Limits {
        bytes: (SIDE_LIMIT * SIDE_LIMIT * 8) as usize,
    });
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let info = reader.info();
    if info.width == 0
        || info.height == 0
        || info.width > SIDE_LIMIT
        || info.height > SIDE_LIMIT
        || info.animation_control.is_some()
    {
        return None;
    }
    let size = reader.output_buffer_size()?;
    if size > (SIDE_LIMIT * SIDE_LIMIT * 4) as usize {
        return None;
    }
    let mut buffer = vec![0; size];
    let frame = reader.next_frame(&mut buffer).ok()?;
    let pixels = &buffer[..frame.buffer_size()];
    let mut rgba = Vec::with_capacity((frame.width * frame.height * 4) as usize);
    match frame.color_type {
        png::ColorType::Rgba => rgba.extend_from_slice(pixels),
        png::ColorType::Rgb => {
            for p in pixels.chunks_exact(3) {
                rgba.extend_from_slice(&[p[0], p[1], p[2], 255]);
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for p in pixels.chunks_exact(2) {
                rgba.extend_from_slice(&[p[0], p[0], p[0], p[1]]);
            }
        }
        png::ColorType::Grayscale => {
            for &v in pixels {
                rgba.extend_from_slice(&[v, v, v, 255]);
            }
        }
        png::ColorType::Indexed => return None,
    }
    Some((frame.width, frame.height, rgba))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(width: u32, height: u32, pixels: &[u8]) -> String {
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header().unwrap().write_image_data(pixels).unwrap();
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    #[test]
    fn transparent_pixels_are_decoded_without_files() {
        let pixels = [10, 20, 30, 0, 40, 50, 60, 128];
        assert_eq!(decode(&png(2, 1, &pixels)), Some((2, 1, pixels.to_vec())));
    }

    #[test]
    fn rejects_corruption_oversize_and_non_png_inputs() {
        assert!(decode("not base64").is_none());
        assert!(decode(&"A".repeat(ENCODED_LIMIT + 1)).is_none());
        assert!(decode(&base64::engine::general_purpose::STANDARD.encode(b"not a PNG")).is_none());
        assert!(decode(&png(SIDE_LIMIT + 1, 1, &vec![0; (SIDE_LIMIT as usize + 1) * 4])).is_none());
        let data = png(1, 1, &[1, 2, 3, 4]);
        assert!(decode(&data[..data.len() / 2]).is_none());
    }

    #[test]
    fn rejects_animated_pngs() {
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_animated(1, 0).unwrap();
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&[1, 2, 3, 4])
            .unwrap();
        assert!(decode(&base64::engine::general_purpose::STANDARD.encode(bytes)).is_none());
    }

    #[test]
    fn expands_palette_transparency() {
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, 2, 1);
        encoder.set_color(png::ColorType::Indexed);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_palette(vec![10, 20, 30, 40, 50, 60]);
        encoder.set_trns(vec![0, 128]);
        encoder.write_header().unwrap().write_image_data(&[0, 1]).unwrap();
        assert_eq!(
            decode(&base64::engine::general_purpose::STANDARD.encode(bytes)),
            Some((2, 1, vec![10, 20, 30, 0, 40, 50, 60, 128]))
        );
    }

    #[test]
    fn cache_reuses_pixels_and_keeps_changed_bytes_distinct() {
        let mut cache = Images::default();
        let one = png(1, 1, &[1, 2, 3, 4]);
        let a = cache.get(&one).unwrap();
        assert!(Arc::ptr_eq(&a, &cache.get(&one).unwrap()));
        let b = cache.get(&png(1, 1, &[1, 2, 3, 5])).unwrap();
        assert!(!Arc::ptr_eq(&a, &b));
        assert_eq!(cache.entries.len(), 2);
    }

    #[test]
    fn cache_bounds_failed_and_successful_inputs() {
        let mut cache = Images::default();
        for n in 0..CACHE_LIMIT * 2 {
            cache.get(&format!("invalid-{n}"));
        }
        assert_eq!(cache.entries.len(), CACHE_LIMIT);
        for n in 0..CACHE_LIMIT * 2 {
            cache.get(&png(1, 1, &[n as u8, 0, 0, 255]));
        }
        assert_eq!(cache.entries.len(), CACHE_LIMIT);
        cache.get(&"A".repeat(ENCODED_LIMIT + 1));
        assert_eq!(cache.entries.len(), CACHE_LIMIT);
        let released = cache.clear();
        assert_eq!(
            released.len(),
            CACHE_LIMIT * 2,
            "evicted GPU images must remain available for explicit atlas removal"
        );
        assert!(cache.entries.is_empty() && cache.retired().is_empty());
    }
    #[test]
    #[ignore = "cross-language WARP publisher probe; input on stdin"]
    fn helper_view_reaches_the_memory_renderer() {
        use std::io::Read;
        let mut raw = String::new();
        std::io::stdin().read_to_string(&mut raw).unwrap();
        assert!(!raw.contains("game_texture"));
        let view: swofty_core::view::View = serde_json::from_str(&raw).unwrap();
        let mut cache = Images::default();
        let (mut archive, mut runtime) = (0, 0);
        for block in crate::blocks::view_blocks(&view) {
            if let swofty_core::view::Block::Table(table) = block {
                assert_eq!(table.columns[1].key(), "team");
                for row in &table.rows {
                    let cell = row.cells()[1].spec();
                    let data = cell.image_data.expect("add-on must supply every image as PNG");
                    let (w, h, _) = decode(&data.png).unwrap();
                    let image = cache.get(&data.png).unwrap();
                    match (w, h) {
                        (24, 24) => archive += 1,
                        (64, 32) => {
                            runtime += 1;
                            for p in image.as_bytes(0).unwrap().chunks_exact(4) {
                                assert!(
                                    (i16::from(p[0]) - 191).abs() <= 1
                                        && (i16::from(p[1]) - 128).abs() <= 1
                                        && (i16::from(p[2]) - 64).abs() <= 1
                                        && (i16::from(p[3]) - 128).abs() <= 1
                                );
                            }
                        }
                        _ => panic!("unexpected fixture dimensions"),
                    }
                }
            }
        }
        assert_eq!((archive, runtime), (12, 12));
        println!("PASS: 12 archive and 12 runtime PNGs reached the generic memory renderer");
    }
}
