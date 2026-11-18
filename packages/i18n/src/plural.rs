//! Which plural form a number takes. Rules follow the CLDR plural rules for whole
//! numbers; add a language by giving it its categories and a rule.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Zero,
    One,
    Two,
    Few,
    Many,
    Other,
}

/// The language part of a code: `zh-CN` is `zh`.
fn base(code: &str) -> &str {
    code.split(['-', '_']).next().unwrap_or(code)
}

/// The forms `code` has, in the order a locale file lists them.
pub fn categories(code: &str) -> &'static [Category] {
    use Category::*;
    match base(code) {
        "ja" | "zh" | "ko" | "vi" | "th" | "id" | "ms" => &[Other],
        "fr" | "pt" => &[One, Many, Other],
        "ru" | "uk" | "be" | "pl" => &[One, Few, Many],
        "cs" | "sk" => &[One, Few, Other],
        "ar" => &[Zero, One, Two, Few, Many, Other],
        _ => &[One, Other],
    }
}

/// The form `n` takes in `code`.
pub fn category(code: &str, n: u64) -> Category {
    use Category::*;
    let (m10, m100) = (n % 10, n % 100);
    match base(code) {
        "ja" | "zh" | "ko" | "vi" | "th" | "id" | "ms" => Other,
        "fr" | "pt" => match n {
            0 | 1 => One,
            _ if n.is_multiple_of(1_000_000) => Many,
            _ => Other,
        },
        "ru" | "uk" | "be" => match (m10, m100) {
            (1, r) if r != 11 => One,
            (2..=4, r) if !(12..=14).contains(&r) => Few,
            _ => Many,
        },
        "pl" => match (n, m10, m100) {
            (1, ..) => One,
            (_, 2..=4, r) if !(12..=14).contains(&r) => Few,
            _ => Many,
        },
        "cs" | "sk" => match n {
            1 => One,
            2..=4 => Few,
            _ => Other,
        },
        "ar" => match (n, m100) {
            (0, _) => Zero,
            (1, _) => One,
            (2, _) => Two,
            (_, 3..=10) => Few,
            (_, 11..=99) => Many,
            _ => Other,
        },
        _ => {
            if n == 1 {
                One
            } else {
                Other
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Category::*;
    use super::*;

    #[test]
    fn english_has_one_and_other() {
        assert_eq!(category("en", 1), One);
        assert_eq!(category("en", 0), Other);
        assert_eq!(category("en", 2), Other);
        assert_eq!(category("de-AT", 1), One);
    }

    #[test]
    fn russian_follows_the_ones_and_teens() {
        let forms: Vec<_> = [1, 2, 4, 5, 11, 12, 14, 21, 22, 25, 101, 111, 0]
            .map(|n| category("ru", n))
            .into();
        assert_eq!(
            forms,
            [
                One, Few, Few, Many, Many, Many, Many, One, Few, Many, One, Many, Many
            ]
        );
    }

    #[test]
    fn polish_one_is_only_one() {
        assert_eq!(category("pl", 1), One);
        assert_eq!(category("pl", 21), Many);
        assert_eq!(category("pl", 22), Few);
        assert_eq!(category("pl", 12), Many);
    }

    #[test]
    fn chinese_never_changes_form() {
        assert_eq!(category("zh-CN", 1), Other);
        assert_eq!(category("zh-CN", 7), Other);
        assert_eq!(categories("zh-CN"), [Other]);
    }

    #[test]
    fn korean_never_changes_form() {
        assert_eq!(category("ko", 1), Other);
        assert_eq!(category("ko", 7), Other);
        assert_eq!(categories("ko"), [Other]);
    }

    #[test]
    fn brazilian_portuguese_counts_zero_and_one_as_one() {
        assert_eq!(categories("pt-BR"), [One, Many, Other]);
        assert_eq!(category("pt-BR", 0), One);
        assert_eq!(category("pt-BR", 1), One);
        assert_eq!(category("pt-BR", 2), Other);
        assert_eq!(category("pt-BR", 1_000_000), Many);
        assert_eq!(category("pt-BR", 1_000_001), Other);
    }

    #[test]
    fn czech_and_arabic_and_french() {
        assert_eq!(
            (category("cs", 1), category("cs", 3), category("cs", 5)),
            (One, Few, Other)
        );
        assert_eq!(categories("ar").len(), 6);
        assert_eq!(
            (
                category("ar", 0),
                category("ar", 2),
                category("ar", 7),
                category("ar", 20),
                category("ar", 100)
            ),
            (Zero, Two, Few, Many, Other)
        );
        assert_eq!(
            (category("fr", 0), category("fr", 1), category("fr", 2)),
            (One, One, Other)
        );
    }

    #[test]
    fn every_category_a_rule_returns_is_listed() {
        for code in ["en", "fr", "ru", "pl", "cs", "ar", "zh", "ja", "ko", "pt-BR"] {
            for n in 0..1000 {
                assert!(categories(code).contains(&category(code, n)), "{code} {n}");
            }
        }
    }
}
