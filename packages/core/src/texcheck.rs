use swofty_formats::erp::Resource;

const SURFACE: &str = "GfxSurfaceRes";
const HEAD_LEN: usize = 40;
const FORMAT: usize = 8;
const WIDTH: usize = 12;
const HEIGHT: usize = 16;
const DEPTH: usize = 20;
const MIPS: usize = 24;
const SLICES: usize = 28;
const VIEW_COUNT: usize = 0x14;
const MAX_SLICES: u32 = 2048;
pub const LARGE: usize = 64 << 20;

const BLOCK8: [u32; 3] = [52, 54, 63];
const BLOCK16: [u32; 5] = [55, 57, 65, 69, 70];
const KNOWN_OTHER: [u32; 5] = [15, 51, 61, 62, 67];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Invalid,
    Suspect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub severity: Severity,
    pub resource: String,
    pub why: String,
}

impl std::fmt::Display for Finding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.resource, self.why)
    }
}

fn word(head: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(head[at..at + 4].try_into().expect("4 bytes"))
}

fn blocks(width: u32, height: u32, mips: u32) -> u64 {
    (0..mips.min(32))
        .map(|m| {
            let w = (width >> m).max(1).div_ceil(4) as u64;
            let h = (height >> m).max(1).div_ceil(4) as u64;
            w * h
        })
        .sum()
}

pub fn block_bytes(format: u32) -> Option<u64> {
    if BLOCK8.contains(&format) {
        Some(8)
    } else if BLOCK16.contains(&format) {
        Some(16)
    } else {
        None
    }
}

pub fn view_name(image: &str) -> Option<&str> {
    image.strip_suffix(".image").filter(|v| v.ends_with(".texarray"))
}

pub fn check_surface(res: &Resource, view: Option<&Resource>) -> Vec<Finding> {
    if res.kind() != SURFACE {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut add = |severity, why: String| {
        out.push(Finding {
            severity,
            resource: res.name.clone(),
            why,
        })
    };
    let Some(head) = res.fragments.first().and_then(|f| f.data().ok()) else {
        return out;
    };
    if head.len() < HEAD_LEN {
        return out;
    }
    let (format, width, height) = (word(&head, FORMAT), word(&head, WIDTH), word(&head, HEIGHT));
    let (depth, mips, slices) = (word(&head, DEPTH), word(&head, MIPS), word(&head, SLICES));
    let shape = format!("format {format}, {width}x{height}, {mips} mip(s), {slices} slice(s)");
    let block = block_bytes(format);
    if block.is_none() && !KNOWN_OTHER.contains(&format) {
        add(
            Severity::Suspect,
            format!("{shape}: the game's own textures never use format {format}"),
        );
    }
    if width == 0 || height == 0 {
        add(Severity::Invalid, format!("{shape}: a side is zero"));
        return out;
    }
    if block.is_some() && (width % 4 != 0 || height % 4 != 0) {
        add(
            Severity::Invalid,
            format!("{shape}: a block compressed texture needs both sides to be a multiple of 4"),
        );
    }
    let longest = 32 - width.max(height).leading_zeros();
    if mips == 0 || mips > longest {
        add(
            Severity::Invalid,
            format!("{shape}: a {width}x{height} texture has 1 to {longest} mips"),
        );
        return out;
    }
    if slices == 0 || slices > MAX_SLICES {
        add(
            Severity::Invalid,
            format!("{shape}: an array holds 1 to {MAX_SLICES} slices"),
        );
        return out;
    }
    if let (Some(block), Some(pixels)) = (block, res.fragments.get(1)) {
        let want = blocks(width, height, mips) * block * u64::from(slices) * u64::from(depth.max(1));
        let have = if pixels.size > 0 {
            pixels.size
        } else {
            pixels.data().map_or(0, |d| d.len() as u64)
        };
        if have != want {
            add(
                Severity::Suspect,
                format!("{shape}: the pixel data is {have} byte(s), the header describes {want}"),
            );
        }
        if have as usize >= LARGE {
            add(
                Severity::Suspect,
                format!("{shape}: {} MiB in one placed resource", have as usize >> 20),
            );
        }
    }
    if let Some(view) = view
        && let Some(v) = view.fragments.first().and_then(|f| f.data().ok())
        && v.len() >= VIEW_COUNT + 4
        && word(&v, VIEW_COUNT) != slices
    {
        add(
            Severity::Invalid,
            format!(
                "{shape}: its view {} says {} slice(s)",
                view.name,
                word(&v, VIEW_COUNT)
            ),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use swofty_formats::erp::Fragment;

    fn fragment(raw: Vec<u8>) -> Fragment {
        Fragment {
            name: *b"temp",
            offset: 0,
            size: raw.len() as u64,
            flags: 0,
            compression: swofty_formats::erp::STORED,
            packed: raw.len() as u64,
            raw,
        }
    }

    fn surface(name: &str, dims: [u32; 5], data: usize) -> Resource {
        let [format, width, height, mips, slices] = dims;
        let mut head = vec![0u8; HEAD_LEN];
        for (at, v) in [
            (0, 2),
            (FORMAT, format),
            (WIDTH, width),
            (HEIGHT, height),
            (DEPTH, 1),
            (MIPS, mips),
            (SLICES, slices),
        ] {
            head[at..at + 4].copy_from_slice(&v.to_le_bytes());
        }
        let mut kind = [0u8; 16];
        kind[..SURFACE.len()].copy_from_slice(SURFACE.as_bytes());
        Resource {
            name: name.into(),
            kind,
            unk: vec![0; 4],
            fragments: vec![fragment(head), fragment(vec![0; data])],
            hash: [0; 16],
        }
    }

    fn view(name: &str, count: u32) -> Resource {
        let mut v = vec![0u8; 128];
        v[VIEW_COUNT..VIEW_COUNT + 4].copy_from_slice(&count.to_le_bytes());
        Resource {
            name: name.into(),
            kind: [0; 16],
            unk: vec![0; 4],
            fragments: vec![fragment(v)],
            hash: [0; 16],
        }
    }

    fn invalid(f: &[Finding]) -> Vec<&Finding> {
        f.iter().filter(|f| f.severity == Severity::Invalid).collect()
    }

    #[test]
    fn a_stock_shaped_name_texture_passes() {
        let r = surface("a.tif.image", [70, 512, 64, 5, 1], 43648);
        assert!(check_surface(&r, None).is_empty());
    }

    #[test]
    fn a_texture_array_with_a_matching_view_passes() {
        let r = surface("u/x.texarray.image", [70, 64, 64, 1, 10], 40960);
        assert!(check_surface(&r, Some(&view("u/x.texarray", 10))).is_empty());
    }

    #[test]
    fn block_compressed_sides_must_be_a_multiple_of_four() {
        let r = surface("a.image", [70, 70, 64, 1, 1], 18 * 16 * 16);
        let f = check_surface(&r, None);
        assert_eq!(invalid(&f).len(), 1);
        assert!(f[0].why.contains("multiple of 4"));
        assert_eq!(f[0].resource, "a.image");
    }

    #[test]
    fn mip_count_cannot_pass_the_longest_side() {
        let r = surface("a.image", [70, 64, 8, 8, 1], 16);
        assert!(invalid(&check_surface(&r, None))[0].why.contains("1 to 7 mips"));
        let r = surface("a.image", [70, 64, 64, 0, 1], 16);
        assert_eq!(invalid(&check_surface(&r, None)).len(), 1);
    }

    #[test]
    fn mip_chains_below_the_block_size_are_valid() {
        let r = surface("a.image", [55, 8, 8, 4, 1], (4 + 1 + 1 + 1) * 16);
        assert!(check_surface(&r, None).is_empty());
    }

    #[test]
    fn slice_count_must_be_in_range_and_agree_with_the_view() {
        let r = surface("x.texarray.image", [70, 64, 64, 1, 3000], 0);
        assert_eq!(invalid(&check_surface(&r, None)).len(), 1);
        let r = surface("x.texarray.image", [70, 64, 64, 1, 10], 40960);
        let f = check_surface(&r, Some(&view("x.texarray", 9)));
        assert_eq!(invalid(&f).len(), 1);
        assert!(f[0].why.contains("x.texarray says 9"));
    }

    #[test]
    fn pixel_data_that_differs_from_the_header_is_suspect() {
        let r = surface("a.image", [70, 64, 64, 1, 10], 40000);
        let f = check_surface(&r, None);
        assert_eq!((f.len(), f[0].severity), (1, Severity::Suspect));
        assert!(f[0].why.contains("40000"));
    }

    #[test]
    fn unknown_formats_and_other_resources_are_handled() {
        let r = surface("a.image", [99, 64, 64, 1, 1], 0);
        assert_eq!(check_surface(&r, None)[0].severity, Severity::Suspect);
        let mut other = surface("a.image", [70, 70, 64, 1, 1], 0);
        other.kind = [0; 16];
        assert!(check_surface(&other, None).is_empty());
    }

    #[test]
    fn view_names_follow_the_image() {
        assert_eq!(view_name("u/x.texarray.image"), Some("u/x.texarray"));
        assert_eq!(view_name("u/x.tif.image"), None);
    }
}
