#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Step {
    Full,
    TightTabs,
    SmallAccount,
    AccountIconOnly,
    LogoOnly,
    Collapsed,
}

impl Step {
    pub(super) fn tight(self) -> bool {
        self != Self::Full
    }

    pub(super) fn account(self) -> Account {
        match self {
            Self::Full | Self::TightTabs => Account::Full,
            Self::SmallAccount => Account::Small,
            _ => Account::IconOnly,
        }
    }

    pub(super) fn wordmark(self) -> bool {
        !matches!(self, Self::LogoOnly | Self::Collapsed)
    }

    pub(super) fn collapsed(self) -> bool {
        self == Self::Collapsed
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Account {
    Full,
    Small,
    IconOnly,
}

const GAP: f32 = 16.;
const LOGO: f32 = 14. + 42.;
const WORDMARK: f32 = 10. + 84.;
const ACTIONS: f32 = 44. + 44. + 4. + 6.;
const TAB_ICON_PAD: f32 = 44.;
const TAB_ICON_PAD_TIGHT: f32 = 20.;
const CHAR: f32 = 7.4;

pub(super) fn tabs_width(labels: &[&str], tight: bool) -> f32 {
    let pad = if tight { TAB_ICON_PAD_TIGHT } else { TAB_ICON_PAD };
    labels
        .iter()
        .map(|l| (l.chars().count() as f32 * CHAR).max(26.) + pad)
        .sum::<f32>()
        + 2. * labels.len().saturating_sub(1) as f32
}

fn account_width(account: Account, signed_in: Option<usize>, bar: f32) -> f32 {
    match (account, signed_in) {
        (Account::Full, None) => 40. + 28. + 19. * CHAR,
        (Account::Small, None) => 40. + 28. + 6. * CHAR,
        (Account::IconOnly, None) => 40. + 18.,
        (Account::Full, Some(n)) => bar + 16. + 24. + 18. + (n as f32 * 8.).min(190.),
        (Account::Small, Some(n)) => bar + 16. + 24. + 18. + (n as f32 * 8.).min(110.),
        (Account::IconOnly, Some(_)) => bar + 8.,
    }
}

pub(super) fn choose(width: f32, labels: &[&str], signed_in: Option<usize>, bar: f32) -> Step {
    for step in [
        Step::Full,
        Step::TightTabs,
        Step::SmallAccount,
        Step::AccountIconOnly,
        Step::LogoOnly,
    ] {
        let left = LOGO + if step.wordmark() { WORDMARK } else { 0. };
        let right = ACTIONS + account_width(step.account(), signed_in, bar);
        let tabs = tabs_width(labels, step.tight());
        if width >= tabs + 2. * left.max(right) + 2. * GAP {
            return step;
        }
    }
    Step::Collapsed
}

#[cfg(test)]
mod tests {
    use super::*;

    const LABELS: [&str; 4] = ["Home", "Library", "Discover", "Time Trial"];

    #[test]
    fn a_wide_window_keeps_everything() {
        assert_eq!(choose(1440., &LABELS, None, 68.), Step::Full);
        assert_eq!(choose(1440., &LABELS, Some(7), 68.), Step::Full);
    }

    #[test]
    fn it_degrades_one_step_at_a_time_as_the_window_narrows() {
        let mut last = 0;
        let order = [
            Step::Full,
            Step::TightTabs,
            Step::SmallAccount,
            Step::AccountIconOnly,
            Step::LogoOnly,
            Step::Collapsed,
        ];
        for width in (300..1500).rev().step_by(10) {
            let at = order
                .iter()
                .position(|s| *s == choose(width as f32, &LABELS, None, 68.))
                .unwrap();
            assert!(at >= last, "{width}");
            last = at;
        }
        assert_eq!(last, order.len() - 1);
    }

    #[test]
    fn the_tabs_never_overlap_what_is_beside_them_unless_collapsed() {
        for width in (320..1500).step_by(5) {
            let step = choose(width as f32, &LABELS, None, 68.);
            if step.collapsed() {
                continue;
            }
            let left = LOGO + if step.wordmark() { WORDMARK } else { 0. };
            let right = ACTIONS + account_width(step.account(), None, 68.);
            let tabs = tabs_width(&LABELS, step.tight());
            let half = width as f32 / 2.;
            assert!(half - tabs / 2. >= left + GAP - 0.01, "{width}");
            assert!(half + tabs / 2. <= width as f32 - right - GAP + 0.01, "{width}");
        }
    }

    #[test]
    fn a_window_that_took_the_dropdown_before_now_keeps_its_tabs() {
        assert_ne!(choose(1000., &LABELS, None, 68.), Step::Collapsed);
        assert!(choose(560., &LABELS, None, 68.) == Step::Collapsed);
    }
}
