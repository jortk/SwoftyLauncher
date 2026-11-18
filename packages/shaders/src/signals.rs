//! The signal registry shared with the game core (`postfx::SIGNALS`): a signal's index here is its index there.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalKind {
    Bool,
    Name,
    Number,
    Event,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Signal {
    pub id: &'static str,
    pub kind: SignalKind,
    /// The names of a `Name` signal, in index order.
    pub values: &'static [&'static str],
    /// The unit of a number signal or an event's magnitude; empty for none.
    pub unit: &'static str,
    /// Typical range of a number signal, for sliders.
    pub min: f64,
    pub max: f64,
    /// An event's magnitude range; `None` when the event carries no magnitude.
    pub magnitude: Option<(f64, f64)>,
    pub in_game: bool,
    pub doc: &'static str,
}

impl Signal {
    pub fn kind_word(&self) -> &'static str {
        match self.kind {
            SignalKind::Bool => "a yes/no signal",
            SignalKind::Name => "a name",
            SignalKind::Number => "a number",
            SignalKind::Event => "an event",
        }
    }

    pub fn per_wheel(&self) -> bool {
        self.id == "car.surface"
    }
}

const CAMERAS: &[&str] = &[
    "cockpit",
    "tcam",
    "tcam_offset",
    "chase_near",
    "chase_far",
    "other",
];
const SURFACES: &[&str] = &["tarmac", "kerb", "rumble", "grass", "gravel", "sand"];
const FLAGS: &[&str] = &["none", "green", "blue", "yellow", "red"];
const SAFETY: &[&str] = &["none", "sc", "vsc", "formation"];
const WEATHER: &[&str] = &[
    "clear",
    "light_cloud",
    "overcast",
    "light_rain",
    "heavy_rain",
    "storm",
];

const fn flag(id: &'static str, in_game: bool, doc: &'static str) -> Signal {
    Signal {
        id,
        kind: SignalKind::Bool,
        values: &[],
        unit: "",
        min: 0.0,
        max: 1.0,
        magnitude: None,
        in_game,
        doc,
    }
}

const fn name(id: &'static str, values: &'static [&'static str], in_game: bool, doc: &'static str) -> Signal {
    Signal {
        id,
        kind: SignalKind::Name,
        values,
        unit: "",
        min: 0.0,
        max: 0.0,
        magnitude: None,
        in_game,
        doc,
    }
}

const fn number(
    id: &'static str,
    unit: &'static str,
    min: f64,
    max: f64,
    in_game: bool,
    doc: &'static str,
) -> Signal {
    Signal {
        id,
        kind: SignalKind::Number,
        values: &[],
        unit,
        min,
        max,
        magnitude: None,
        in_game,
        doc,
    }
}

const fn event(
    id: &'static str,
    unit: &'static str,
    magnitude: Option<(f64, f64)>,
    in_game: bool,
    doc: &'static str,
) -> Signal {
    Signal {
        id,
        kind: SignalKind::Event,
        values: &[],
        unit,
        min: 0.0,
        max: 0.0,
        magnitude,
        in_game,
        doc,
    }
}

/// Every signal; the position is the signal's index.
pub static SIGNALS: [Signal; 31] = [
    name(
        "view.camera",
        CAMERAS,
        false,
        "The camera view: cockpit, tcam, tcam_offset, chase_near, chase_far or other.",
    ),
    flag("view.replay", true, "On while a replay plays."),
    flag("view.paused", true, "On while the game is paused."),
    flag("view.photo", true, "On in photo mode."),
    flag(
        "view.menu",
        true,
        "On in the menus and on loading screens: whenever no session is running.",
    ),
    flag("view.cutscene", true, "On during a cutscene."),
    flag("view.spectating", true, "On while spectating another car."),
    event("session.start", "", None, true, "Fires when the session loads."),
    event("race.start", "", None, true, "Fires at lights out."),
    event(
        "session.lap",
        "",
        None,
        true,
        "Fires when the player completes a lap.",
    ),
    event("session.end", "", None, true, "Fires when the session ends."),
    number("car.speed", "km/h", 0.0, 400.0, true, "The car's speed in km/h."),
    number("car.rpm", "", 0.0, 1.0, true, "Engine revs, 0-1 of the limiter."),
    number(
        "car.gear",
        "",
        -1.0,
        8.0,
        true,
        "The gear: -1 reverse, 0 neutral, 1-8.",
    ),
    number("car.throttle", "", 0.0, 1.0, true, "Throttle pedal, 0-1."),
    number("car.brake", "", 0.0, 1.0, true, "Brake pedal, 0-1."),
    number("car.steer", "", -1.0, 1.0, true, "Steering, -1 left to 1 right."),
    number(
        "car.g_lateral",
        "g",
        -6.0,
        6.0,
        true,
        "Lateral acceleration in g.",
    ),
    number(
        "car.g_long",
        "g",
        -6.0,
        6.0,
        true,
        "Longitudinal acceleration in g, braking negative.",
    ),
    name(
        "car.surface",
        SURFACES,
        true,
        "The surface under each wheel: tarmac, kerb, rumble, grass, gravel or sand. `wheels` sets how many wheels must match.",
    ),
    event(
        "car.impact",
        "g",
        Some((0.0, 20.0)),
        false,
        "Fires on a collision; the magnitude is the hit in g.",
    ),
    event(
        "car.damage",
        "",
        Some((0.0, 1.0)),
        false,
        "Fires when a part takes damage; the magnitude is the increase, 0-1.",
    ),
    flag("car.in_pit_lane", false, "On while the car is in the pit lane."),
    flag(
        "car.off_track",
        false,
        "On while all four wheels are off the track.",
    ),
    name(
        "session.flag",
        FLAGS,
        false,
        "The flag shown: none, green, blue, yellow or red.",
    ),
    name("session.safety_car", SAFETY, false, "none, sc, vsc or formation."),
    name(
        "session.weather",
        WEATHER,
        false,
        "clear, light_cloud, overcast, light_rain, heavy_rain or storm.",
    ),
    number(
        "session.wetness",
        "",
        0.0,
        1.0,
        true,
        "How wet the track is, 0 dry to 1 soaked.",
    ),
    event(
        "session.flashback",
        "",
        None,
        false,
        "Fires when the player uses a flashback.",
    ),
    event(
        "session.chequered",
        "",
        None,
        false,
        "Fires when the player takes the chequered flag.",
    ),
    number(
        "car.wetness",
        "",
        0.0,
        1.0,
        true,
        "Water on the track under the car's wheels, 0-1: the wettest of the four.",
    ),
];

/// The signal with this id and its index.
pub fn signal(id: &str) -> Option<(usize, &'static Signal)> {
    SIGNALS.iter().enumerate().find(|(_, s)| s.id == id)
}
