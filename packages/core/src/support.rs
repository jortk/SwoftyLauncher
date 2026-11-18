//! Support: the donation goal, the supporters and the signed-in user's own support, from the launcher's server.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::timetrial::api::{Api, ApiError};
use crate::timetrial::boards::Rows;

pub const BIO_MAX: usize = 140;
pub const FALLBACK_URL: &str = "https://donate.swofty.net/";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Goal {
    #[serde(default)]
    pub goal_cents: u64,
    #[serde(default)]
    pub raised_cents: u64,
    #[serde(default)]
    pub since: String,
    #[serde(default)]
    pub costs: Vec<String>,
}

impl Goal {
    pub fn fraction(&self) -> f32 {
        match self.goal_cents {
            0 => 0.,
            g => (self.raised_cents as f32 / g as f32).clamp(0., 1.),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Me {
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub tier: Option<String>,
    #[serde(default)]
    pub months_left: Option<u32>,
    #[serde(default)]
    pub expires_at: Option<Value>,
    #[serde(default)]
    pub bio: Option<String>,
    #[serde(default)]
    pub display_mode: Option<String>,
    /// Whether the launcher shows this user no ads: supporters, admins, Swofty
    /// Launcher Support and addon creators. Servers without it leave it false.
    #[serde(default)]
    pub ad_free: bool,
    /// RevIQ's hash of the user's Discord email, for ad matching. Only users
    /// who see ads and signed in with the email scope have one.
    #[serde(default)]
    pub ad_uid: Option<String>,
}

impl Me {
    pub fn sees_ads(&self) -> bool {
        !(self.active || self.ad_free)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Display {
    #[serde(default)]
    pub anonymous: bool,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Supporter {
    pub tier: String,
    #[serde(default)]
    pub bio: Option<String>,
    pub display: Display,
}

impl Supporter {
    pub fn plus(&self) -> bool {
        self.tier == "supporter_plus"
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Claimed {
    #[serde(default)]
    pub tier: String,
    #[serde(default)]
    pub months_left: u32,
    #[serde(default)]
    pub expires_at: Option<Value>,
    #[serde(default)]
    pub role_granted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Supporter,
    SupporterPlus,
    OneTime,
}

impl Kind {
    fn key(self) -> &'static str {
        match self {
            Self::Supporter => "supporter",
            Self::SupporterPlus => "supporter_plus",
            Self::OneTime => "onetime",
        }
    }
}

pub fn tier_name(tier: &str) -> &'static str {
    match tier {
        "supporter_plus" => "Supporter+",
        _ => "Supporter",
    }
}

/// `YYYY-MM-DD` for an expiry given as unix seconds or as a date text.
pub fn format_date(at: &Value) -> String {
    match at {
        Value::Number(n) => {
            let z = n.as_i64().unwrap_or(0).div_euclid(86_400) + 719_468;
            let era = z.div_euclid(146_097);
            let doe = z.rem_euclid(146_097);
            let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
            let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
            let mp = (5 * doy + 2) / 153;
            let d = doy - (153 * mp + 2) / 5 + 1;
            let m = if mp < 10 { mp + 3 } else { mp - 9 };
            format!("{:04}-{m:02}-{d:02}", yoe + era * 400 + i64::from(m <= 2))
        }
        Value::String(s) => s.chars().take(10).collect(),
        _ => String::new(),
    }
}

/// Whole dollars typed by the user as cents: `"25"` or `"25.50"`; `None` for anything else or zero.
pub fn dollars_to_cents(text: &str) -> Option<u64> {
    let text = text.trim().trim_start_matches('$');
    let (whole, frac) = text.split_once('.').unwrap_or((text, ""));
    if whole.is_empty() && frac.is_empty()
        || frac.len() > 2
        || !whole.chars().chain(frac.chars()).all(|c| c.is_ascii_digit())
    {
        return None;
    }
    let whole: u64 = if whole.is_empty() { 0 } else { whole.parse().ok()? };
    let frac: u64 = format!("{frac:0<2}").parse().ok()?;
    let cents = whole.checked_mul(100)?.checked_add(frac)?;
    (cents > 0).then_some(cents)
}

impl Api {
    pub fn support_goal(&self) -> Result<Goal, ApiError> {
        self.get_json("/v1/support/goal")
    }

    pub fn support_me(&self) -> Result<Me, ApiError> {
        self.get_json("/v1/support/me")
    }

    pub fn support_supporters(&self) -> Result<Vec<Supporter>, ApiError> {
        Ok(self
            .get_json::<Rows<Supporter>>("/v1/support/supporters")?
            .into_vec())
    }

    pub fn support_update(&self, bio: Option<&str>, display_mode: Option<&str>) -> Result<(), ApiError> {
        let mut body = serde_json::Map::new();
        if let Some(b) = bio {
            body.insert("bio".into(), b.into());
        }
        if let Some(m) = display_mode {
            body.insert("display_mode".into(), m.into());
        }
        self.post_json("/v1/support/me", &Value::Object(body))
    }

    /// The Stripe page to open for a subscription or a one-time donation.
    pub fn support_checkout(&self, kind: Kind, amount_cents: Option<u64>) -> Result<String, ApiError> {
        let mut body = json!({ "kind": kind.key() });
        if let Some(c) = amount_cents {
            body["amount_cents"] = c.into();
        }
        self.post_value("/v1/support/checkout", &body)?
            .get("url")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| ApiError::network("the server sent no checkout link"))
    }

    pub fn support_claim(&self, stripe_email: &str, payment_id: &str) -> Result<Claimed, ApiError> {
        let value = self.post_value(
            "/v1/support/claim",
            &json!({ "stripe_email": stripe_email, "payment_id": payment_id }),
        )?;
        serde_json::from_value(value)
            .map_err(|e| ApiError::network(format!("the server's answer to the claim is not valid: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dollars_become_cents() {
        assert_eq!(dollars_to_cents("20"), Some(2000));
        assert_eq!(dollars_to_cents(" $7.5 "), Some(750));
        assert_eq!(dollars_to_cents("0.25"), Some(25));
        assert_eq!(dollars_to_cents("0"), None);
        assert_eq!(dollars_to_cents("1.234"), None);
        assert_eq!(dollars_to_cents("abc"), None);
        assert_eq!(dollars_to_cents(""), None);
    }

    #[test]
    fn expiry_reads_seconds_or_text() {
        assert_eq!(format_date(&json!(1_798_761_600)), "2027-01-01");
        assert_eq!(format_date(&json!("2027-03-04T00:00:00Z")), "2027-03-04");
    }

    #[test]
    fn supporters_and_ad_free_users_see_no_ads() {
        let me = |json: &str| serde_json::from_str::<Me>(json).unwrap().sees_ads();
        assert!(me(r#"{"active":false}"#));
        assert!(!me(r#"{"active":true}"#));
        assert!(!me(r#"{"active":false,"ad_free":true}"#));
    }

    #[test]
    fn supporters_read_named_and_anonymous() {
        let rows: Rows<Supporter> = serde_json::from_str(
            r#"{"supporters":[{"tier":"supporter_plus","bio":"hi","display":{"anonymous":true}},
                {"tier":"supporter","bio":"","display":{"name":"X","avatar_url":"u"}}]}"#,
        )
        .unwrap();
        let rows = rows.into_vec();
        assert!(rows[0].plus() && rows[0].display.anonymous);
        assert_eq!(rows[1].display.name.as_deref(), Some("X"));
    }

    #[test]
    fn the_goal_fills_up_to_full() {
        let g = Goal {
            goal_cents: 50_000,
            raised_cents: 12_500,
            since: String::new(),
            costs: Vec::new(),
        };
        assert_eq!(g.fraction(), 0.25);
        assert_eq!(
            Goal {
                raised_cents: 90_000,
                ..g
            }
            .fraction(),
            1.
        );
    }

    #[test]
    fn a_servers_message_is_what_the_user_reads() {
        let e = ApiError::from_body(
            400,
            r#"{"error":"no_payment","message":"No payment found for that email."}"#,
        );
        assert_eq!(e.message, "No payment found for that email.");
    }
}
