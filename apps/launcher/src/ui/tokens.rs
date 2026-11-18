use gpui_kit::{Hsla, Pixels};

use crate::scale::px;
use crate::theme::Palette;

/// Font sizes, unscaled. Use the matching function for a scaled [`Pixels`].
pub mod text {
    use super::*;

    pub const CAPTION: f32 = 11.;
    pub const SMALL: f32 = 12.;
    pub const BODY: f32 = 13.;
    pub const TITLE: f32 = 15.;
    pub const HEADING: f32 = 18.;

    pub fn caption() -> Pixels {
        px(CAPTION)
    }

    pub fn small() -> Pixels {
        px(SMALL)
    }

    pub fn body() -> Pixels {
        px(BODY)
    }

    pub fn title() -> Pixels {
        px(TITLE)
    }

    pub fn heading() -> Pixels {
        px(HEADING)
    }
}

/// Corner radii, unscaled.
pub mod radius {
    use super::*;

    pub const XS: f32 = 4.;
    pub const SM: f32 = 5.;
    pub const MD: f32 = 6.;
    pub const LG: f32 = 10.;
    pub const XL: f32 = 14.;

    pub fn xs() -> Pixels {
        px(XS)
    }

    pub fn sm() -> Pixels {
        px(SM)
    }

    pub fn md() -> Pixels {
        px(MD)
    }

    pub fn lg() -> Pixels {
        px(LG)
    }

    pub fn xl() -> Pixels {
        px(XL)
    }
}

/// Gaps and padding, unscaled.
pub mod space {
    use super::*;

    pub const XS: f32 = 4.;
    pub const SM: f32 = 8.;
    pub const MD: f32 = 12.;
    pub const LG: f32 = 16.;
    pub const XL: f32 = 24.;
    /// The page's left and right padding.
    pub const GUTTER: f32 = 28.;

    pub fn xs() -> Pixels {
        px(XS)
    }

    pub fn sm() -> Pixels {
        px(SM)
    }

    pub fn md() -> Pixels {
        px(MD)
    }

    pub fn lg() -> Pixels {
        px(LG)
    }

    pub fn xl() -> Pixels {
        px(XL)
    }

    pub fn gutter() -> Pixels {
        px(GUTTER)
    }
}

/// How serious a message or badge is; picks its colour.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tone {
    #[default]
    Info,
    Success,
    Warning,
    Error,
}

impl Tone {
    pub fn color(self, p: &Palette) -> Hsla {
        match self {
            Self::Info => p.accent,
            Self::Success => p.success,
            Self::Warning => p.warning,
            Self::Error => p.error,
        }
    }

    pub fn icon(self) -> gpui_kit::assets::IconName {
        use gpui_kit::assets::IconName;
        match self {
            Self::Info => IconName::Info,
            Self::Success => IconName::CircleCheck,
            Self::Warning => IconName::TriangleAlert,
            Self::Error => IconName::CircleX,
        }
    }
}
