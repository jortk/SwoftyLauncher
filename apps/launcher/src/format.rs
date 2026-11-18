//! Numbers, sizes and times as the launcher shows them, so every page words them the same.

use swofty_i18n::{tr, trf, trn};

const KB: f64 = 1024.;

/// `bytes` the way Explorer shows them: whole KB, then MB and GB with decimals.
pub fn size(bytes: u64) -> String {
    let b = bytes as f64;
    if b < KB {
        format!("{bytes} B")
    } else if b < KB * KB {
        format!("{:.0} KB", b / KB)
    } else if b < KB * KB * KB {
        format!("{:.1} MB", b / KB / KB)
    } else {
        format!("{:.2} GB", b / KB / KB / KB)
    }
}

/// `bytes` for disk space: GB from one gigabyte, else MB.
pub fn size_coarse(bytes: u64) -> String {
    if bytes >= 1 << 30 {
        format!("{} GB", swofty_core::disk::gb(bytes))
    } else {
        format!("{} MB", bytes.div_ceil(1 << 20))
    }
}

/// A fraction (0 to 1) as a whole percent: `0.42` is `42%`.
pub fn percent(fraction: f32) -> String {
    format!("{:.0}%", fraction * 100.)
}

/// A count in short form: `950`, `1.2k`, `41k`, `3.4M`.
pub fn compact(n: u64) -> String {
    match n {
        0..1000 => n.to_string(),
        1000..10_000 => format!("{:.1}k", n as f64 / 1000.),
        10_000..1_000_000 => format!("{}k", n / 1000),
        _ => format!("{:.1}M", n as f64 / 1e6),
    }
}

/// A length of time: `12 s`, `5 min`, `2 h 5 min`.
pub fn duration(secs: u64) -> String {
    match secs {
        0..60 => trf!("{count} s", count = secs),
        60..3600 => trf!("{count} min", count = secs / 60),
        _ => trf!("{h} h {m} min", h = secs / 3600, m = secs % 3600 / 60),
    }
}

/// How long ago something was, from the seconds that have passed.
pub fn ago(elapsed_secs: f64) -> String {
    let mins = (elapsed_secs.max(0.) / 60.) as u64;
    match mins {
        0 => tr("just now").to_owned(),
        1..60 => trn(mins, "{count} minute ago", "{count} minutes ago"),
        60..1440 => trn(mins / 60, "{count} hour ago", "{count} hours ago"),
        _ => trn(mins / 1440, "{count} day ago", "{count} days ago"),
    }
}

/// [`ago`] for a Unix timestamp in seconds.
pub fn ago_since(epoch_secs: f64) -> String {
    ago(now_secs() - epoch_secs)
}

fn now_secs() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0., |d| d.as_secs_f64())
}

/// An id as words: `team_simplified` becomes `Team Simplified`.
pub fn title_case(word: &str) -> String {
    word.split(['_', '-'])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().chain(c).collect::<String>())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_step_up_like_explorer() {
        assert_eq!(size(512), "512 B");
        assert_eq!(size(2048), "2 KB");
        assert_eq!(size(5 << 20), "5.0 MB");
        assert_eq!(size(3 << 30), "3.00 GB");
    }

    #[test]
    fn disk_sizes_are_coarse() {
        assert_eq!(size_coarse(0), "0 MB");
        assert_eq!(size_coarse(1), "1 MB");
        assert_eq!(size_coarse(5 << 20), "5 MB");
        assert_eq!(size_coarse(3 << 29), "1.5 GB");
    }

    #[test]
    fn counts_and_percent_are_short() {
        assert_eq!(compact(999), "999");
        assert_eq!(compact(1500), "1.5k");
        assert_eq!(compact(2_500_000), "2.5M");
        assert_eq!(percent(0.426), "43%");
    }

    #[test]
    fn durations_and_ages_read_in_english() {
        assert_eq!(duration(42), "42 s");
        assert_eq!(duration(300), "5 min");
        assert_eq!(duration(7500), "2 h 5 min");
        assert_eq!(ago(10.), "just now");
        assert_eq!(ago(60.), "1 minute ago");
        assert_eq!(ago(5. * 3600.), "5 hours ago");
        assert_eq!(ago(3. * 86_400.), "3 days ago");
    }
}

/// A lap time from milliseconds: `1:31.234`, or `58.815` under a minute.
pub fn lap_time(ms: u64) -> String {
    let (mins, rest) = (ms / 60_000, ms % 60_000);
    match mins {
        0 => format!("{}.{:03}", rest / 1000, rest % 1000),
        _ => format!("{mins}:{:02}.{:03}", rest / 1000, rest % 1000),
    }
}

/// A sector time from milliseconds: `29.001`.
pub fn sector_time(ms: u64) -> String {
    format!("{}.{:03}", ms / 1000, ms % 1000)
}

/// The date part of an ISO-8601 timestamp: `2026-11-10`.
pub fn iso_date(stamp: &str) -> &str {
    stamp.split('T').next().unwrap_or(stamp)
}

#[cfg(test)]
mod lap_tests {
    use super::*;

    #[test]
    fn formats_lap_times() {
        assert_eq!(lap_time(91_234), "1:31.234");
        assert_eq!(lap_time(58_815), "58.815");
        assert_eq!(lap_time(60_005), "1:00.005");
        assert_eq!(sector_time(29_001), "29.001");
        assert_eq!(iso_date("2026-11-10T04:10:11Z"), "2026-11-10");
    }
}
