use std::borrow::Cow;

use gpui_kit::App;

const REGULAR: &[u8] = include_bytes!("../assets/fonts/DejaVuSans.ttf");
const BOLD: &[u8] = include_bytes!("../assets/fonts/DejaVuSans-Bold.ttf");

pub fn install(cx: &mut App) {
    let fonts = vec![Cow::Borrowed(REGULAR), Cow::Borrowed(BOLD)];
    match cx.text_system().add_fonts(fonts) {
        Ok(()) => crate::app_log::step("fallback fonts installed"),
        Err(_) => crate::app_log::step("fallback fonts not installed"),
    }
}
