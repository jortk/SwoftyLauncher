use crate::manifest::AddonManifest;
use crate::stdlib::careers::Career;

pub const TITLE: &str = "This addon has affected your saves";
pub const BODY: &str = "Careers you played with it may not load without it.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavesWarning {
    pub id: String,
    pub name: String,
    pub message: Option<String>,
}

impl SavesWarning {
    pub fn of(m: &AddonManifest) -> Option<Self> {
        m.saves.affects.then(|| Self {
            id: m.id.clone(),
            name: m.name.clone(),
            message: m.saves.message().map(str::to_owned),
        })
    }

    /// Adds to the warning the careers in `needing` (see `stdlib::careers::needing`): they may not
    /// load as saved while the addon is off or gone. `of` gives the manifest's warning, if any, to build on.
    pub fn with_careers(of: Option<Self>, id: &str, name: &str, needing: &[&Career]) -> Option<Self> {
        if needing.is_empty() {
            return of;
        }
        let shown = needing
            .iter()
            .take(3)
            .map(|c| {
                if c.label.is_empty() {
                    c.career.as_str()
                } else {
                    c.label.as_str()
                }
            })
            .collect::<Vec<_>>()
            .join("; ");
        let more = needing.len().saturating_sub(3);
        let more = if more > 0 {
            format!("; {more} more")
        } else {
            String::new()
        };
        let line = format!(
            "{} need{} it: {shown}{more}. They may not load, or may load with stock content in its place, while it is off or removed.",
            match needing.len() {
                1 => "1 career".to_owned(),
                n => format!("{n} careers"),
            },
            if needing.len() == 1 { "s" } else { "" }
        );
        let mut w = of.unwrap_or_else(|| Self {
            id: id.to_owned(),
            name: name.to_owned(),
            message: None,
        });
        w.message = Some(match w.message.take() {
            Some(m) => format!("{m}\n{line}"),
            None => line,
        });
        Some(w)
    }

    pub fn text(&self) -> String {
        let mut text = format!(
            "Warning: {} ({}) has affected your saves. {BODY}",
            self.name, self.id
        );
        if let Some(m) = &self.message {
            text.push('\n');
            text.push_str(m);
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(saves: &str) -> AddonManifest {
        AddonManifest::parse(&format!(
            r#"{{"schema_version":1,"id":"t.saves","name":"Saves","version":"1.0.0","author":"t",
                "compat":{{"game":"f1_25"}}{saves}}}"#
        ))
        .unwrap()
    }

    #[test]
    fn warns_only_for_addons_that_affect_saves() {
        assert_eq!(SavesWarning::of(&manifest("")), None);
        assert_eq!(
            SavesWarning::of(&manifest(r#","saves":{"affects":false,"message":"m"}"#)),
            None
        );
        let w = SavesWarning::of(&manifest(r#","saves":{"affects":true}"#)).unwrap();
        assert_eq!(w.message, None);
        assert_eq!(
            w.text(),
            "Warning: Saves (t.saves) has affected your saves. Careers you played with it may not load without it."
        );
        let w = SavesWarning::of(&manifest(
            r#","saves":{"affects":true,"message":" Retire drivers instead. "}"#,
        ))
        .unwrap();
        assert_eq!(w.message.as_deref(), Some("Retire drivers instead."));
        assert!(w.text().ends_with("without it.\nRetire drivers instead."));
        let blank = SavesWarning::of(&manifest(r#","saves":{"affects":true,"message":"  "}"#)).unwrap();
        assert_eq!(blank.message, None);
    }

    #[test]
    fn names_the_careers_that_need_an_addon() {
        let career = |id: &str, label: &str| Career {
            career: id.into(),
            label: label.into(),
            ..Career::default()
        };
        let (a, b) = (career("a", "My Team"), career("b", ""));
        assert_eq!(SavesWarning::with_careers(None, "x", "X", &[]), None);
        let w = SavesWarning::with_careers(None, "x", "X", &[&a, &b]).unwrap();
        assert_eq!(
            w.message.as_deref(),
            Some(
                "2 careers need it: My Team; b. They may not load, or may load with stock content in its place, while it is off or removed."
            )
        );
        let base = SavesWarning::of(&manifest(r#","saves":{"affects":true,"message":"m"}"#));
        let w = SavesWarning::with_careers(base, "x", "X", &[&a]).unwrap();
        assert!(w.message.unwrap().starts_with("m\n1 career needs it: My Team."));
    }
}
