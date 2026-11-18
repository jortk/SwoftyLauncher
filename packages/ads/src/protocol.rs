use serde::{Deserialize, Serialize};

/// One line the launcher writes to the Linux helper. Positions and sizes are
/// physical pixels inside the launcher window; `scale` is its scale factor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Command {
    Show {
        slot: String,
        url: String,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        scale: f64,
    },
    Hide {
        slot: String,
    },
    /// Destroys the slot's webview; a later `Show` opens it again.
    Close {
        slot: String,
    },
}

/// What the consent page reports while RevIQ's consent form runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Consent {
    /// The form is open and waiting for the player.
    Asking,
    /// The player answered, and the answer is stored.
    Given,
    /// Nothing has to be asked where the player is.
    NotNeeded,
    /// The form never loaded.
    Unavailable,
}

impl Consent {
    /// The page's own message, `{"consent":"asking"}`.
    pub fn from_page(body: &str) -> Option<Self> {
        #[derive(Deserialize)]
        struct Page {
            consent: Consent,
        }
        serde_json::from_str::<Page>(body).ok().map(|p| p.consent)
    }

    /// Whether the player may move on.
    pub fn settled(self) -> bool {
        !matches!(self, Self::Asking)
    }
}

/// One line the Linux helper writes back to the launcher.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    pub slot: String,
    pub consent: Consent,
}

impl Event {
    pub fn line(&self) -> String {
        let mut line = serde_json::to_string(self).unwrap_or_default();
        line.push('\n');
        line
    }

    pub fn parse(line: &str) -> Option<Self> {
        serde_json::from_str(line.trim()).ok()
    }
}

impl Command {
    pub fn line(&self) -> String {
        let mut line = serde_json::to_string(self).unwrap_or_default();
        line.push('\n');
        line
    }

    pub fn parse(line: &str) -> Option<Self> {
        serde_json::from_str(line.trim()).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_survives_a_line() {
        let show = Command::Show {
            slot: "video".into(),
            url: "https://ads.swofty.net/app/video?bg=000000".into(),
            x: 10,
            y: -4,
            width: 400,
            height: 225,
            scale: 1.5,
        };
        let hide = Command::Hide { slot: "video".into() };
        let close = Command::Close { slot: "video".into() };
        for command in [show, hide, close] {
            let line = command.line();
            assert!(line.ends_with('\n') && !line[..line.len() - 1].contains('\n'));
            assert_eq!(Command::parse(&line), Some(command));
        }
    }

    #[test]
    fn the_consent_page_message_reads() {
        assert_eq!(
            Consent::from_page(r#"{"consent":"asking"}"#),
            Some(Consent::Asking)
        );
        assert_eq!(
            Consent::from_page(r#"{"consent":"not_needed"}"#),
            Some(Consent::NotNeeded)
        );
        assert_eq!(Consent::from_page("https://example.com"), None);
        assert_eq!(Consent::from_page(r#"{"consent":"maybe"}"#), None);
        let event = Event {
            slot: "consent".into(),
            consent: Consent::Given,
        };
        assert_eq!(Event::parse(&event.line()), Some(event));
        assert!(!Consent::Asking.settled() && Consent::Unavailable.settled());
    }

    #[test]
    fn a_broken_line_is_ignored() {
        assert_eq!(Command::parse("{\"op\":\"explode\"}"), None);
        assert_eq!(Command::parse("not json"), None);
    }
}
