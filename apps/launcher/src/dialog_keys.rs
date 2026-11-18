use std::rc::Rc;

use gpui_kit::component::WindowExt;
use gpui_kit::*;

const DIALOG: &str = "Dialog";

const ARROWS: [(&str, &str); 4] = [
    ("left", "root::TabPrev"),
    ("up", "root::TabPrev"),
    ("right", "root::Tab"),
    ("down", "root::Tab"),
];

pub(crate) fn init(cx: &mut App) {
    bind_arrows(cx);
    cx.intercept_keystrokes(press_focused).detach();
}

fn press_focused(event: &KeystrokeEvent, window: &mut Window, cx: &mut App) {
    let stroke = &event.keystroke;
    if stroke.key != "enter" || stroke.modifiers.modified() {
        return;
    }
    if !event.context_stack.last().is_some_and(|c| c.contains(DIALOG)) {
        return;
    }
    if !window.focused(cx).is_some_and(|f| f.tab_stop) || !window.has_active_dialog(cx) {
        return;
    }
    cx.stop_propagation();
    window.defer(cx, |window, cx| {
        let keystroke = Keystroke::parse("space").expect("space is a keystroke");
        window.dispatch_event(
            PlatformInput::KeyDown(KeyDownEvent {
                keystroke: keystroke.clone(),
                is_held: false,
                prefer_character_input: false,
            }),
            cx,
        );
        window.dispatch_event(PlatformInput::KeyUp(KeyUpEvent { keystroke }), cx);
    });
}

fn bind_arrows(cx: &mut App) {
    let Ok(context) = KeyBindingContextPredicate::parse(DIALOG).map(Rc::new) else {
        return;
    };
    let mut bindings = Vec::new();
    for (key, action) in ARROWS {
        match cx
            .build_action(action, None)
            .map_err(|e| e.to_string())
            .and_then(|a| {
                KeyBinding::load(key, a, Some(context.clone()), false, None, cx).map_err(|e| e.to_string())
            }) {
            Ok(binding) => bindings.push(binding),
            Err(e) => crate::app_log::note(&format!("ui: no {key} key in dialogs: {e}")),
        }
    }
    cx.bind_keys(bindings);
}
