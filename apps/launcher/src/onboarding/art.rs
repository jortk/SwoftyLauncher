use std::sync::{Arc, LazyLock};

use gpui_kit::*;
use swofty_core::session::UiMode;

use crate::scale::px;

pub(super) const WIDTH: f32 = 300.;
pub(super) const HEIGHT: f32 = 188.;

fn image(bytes: &'static [u8]) -> Arc<Image> {
    Arc::new(Image::from_bytes(ImageFormat::Svg, bytes.to_vec()))
}

fn illustration(mode: UiMode) -> Arc<Image> {
    static PC: LazyLock<Arc<Image>> = LazyLock::new(|| image(include_bytes!("../../assets/display-pc.svg")));
    static TV: LazyLock<Arc<Image>> = LazyLock::new(|| image(include_bytes!("../../assets/display-tv.svg")));
    match mode {
        UiMode::Pc => PC.clone(),
        UiMode::Tv => TV.clone(),
    }
}

pub(super) fn display(mode: UiMode) -> Img {
    img(illustration(mode)).w(px(WIDTH)).h(px(HEIGHT))
}

pub(super) fn globe() -> Img {
    static GLOBE: LazyLock<Arc<Image>> =
        LazyLock::new(|| image(include_bytes!("../../assets/language-globe.svg")));
    img(GLOBE.clone()).w(px(WIDTH * 0.6)).h(px(HEIGHT * 0.6))
}
