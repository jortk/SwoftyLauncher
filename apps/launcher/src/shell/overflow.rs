use gpui_kit::SharedString;

use super::Page;

#[derive(Clone)]
pub(crate) struct Link {
    pub page: Page,
    pub label: SharedString,
    pub count: Option<usize>,
}

impl Link {
    pub(super) fn new(page: Page, count: Option<usize>) -> Self {
        Self {
            page,
            label: page.sub_label().into(),
            count,
        }
    }
}

fn link_width(link: &Link) -> f32 {
    26. + 7.2 * link.label.chars().count() as f32 + link.count.map_or(0., |_| 30.)
}

pub(super) fn split_overflow(mut links: Vec<Link>, fixed: usize, budget: f32) -> (Vec<Link>, Vec<Link>) {
    let mut used: f32 = links.iter().take(fixed).map(link_width).sum();
    let mut keep = fixed.min(links.len());
    let rest = &links[keep..];
    let all_fit = used + rest.iter().map(link_width).sum::<f32>() <= budget;
    for link in rest {
        let w = link_width(link);
        if !all_fit && used + w + 80. > budget {
            break;
        }
        used += w;
        keep += 1;
    }
    let more = links.split_off(keep);
    (links, more)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(label: &str) -> Link {
        Link {
            page: Page::Addons,
            label: label.to_owned().into(),
            count: None,
        }
    }

    #[test]
    fn everything_fits_in_a_wide_window() {
        let links = vec![link("Addons"), link("Shaders"), link("Saves"), link("Importer")];
        let (inline, more) = split_overflow(links, 3, 1200.);
        assert_eq!((inline.len(), more.len()), (4, 0));
    }

    #[test]
    fn the_extra_pages_move_to_more_when_tight() {
        let links = vec![
            link("Addons"),
            link("Shaders"),
            link("Saves"),
            link("Importer"),
            link("DB Editor"),
            link("Grid Editor"),
        ];
        let (inline, more) = split_overflow(links, 3, 330.);
        assert_eq!(inline.len(), 3);
        assert_eq!(more.len(), 3);
    }
}
