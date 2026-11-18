use std::path::Path;

pub const STACK_BUFFER_OVERRUN: u32 = 0xc000_0409;
pub const FAIL_FAST_EXCEPTION: u32 = 0xc000_0602;
pub const INVALID_CRUNTIME_PARAMETER: u32 = 0xc000_0417;

pub const LOADER_TAIL_LINES: usize = 8;

const FAST_FAIL: &[(u32, &str)] = &[
    (
        STACK_BUFFER_OVERRUN,
        "F1 25 stopped itself after detecting a corrupted stack or an invalid call",
    ),
    (
        FAIL_FAST_EXCEPTION,
        "F1 25 stopped itself with a fail-fast request",
    ),
    (
        INVALID_CRUNTIME_PARAMETER,
        "F1 25 stopped itself after its C runtime rejected an invalid argument",
    ),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitClass {
    Clean,
    FastFail,
    Crash,
    Other,
}

pub fn classify(code: i32) -> ExitClass {
    let code = code as u32;
    if is_fast_fail(code) {
        ExitClass::FastFail
    } else if crate::session::crash_code(code) {
        ExitClass::Crash
    } else if code == 0 {
        ExitClass::Clean
    } else {
        ExitClass::Other
    }
}

pub fn is_fast_fail(code: u32) -> bool {
    FAST_FAIL.iter().any(|(c, _)| *c == code)
}

pub fn sentence(code: u32) -> Option<String> {
    FAST_FAIL
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(c, text)| format!("{text} ({c:#010x})"))
}

pub fn last_lines(log: &str, n: usize) -> Vec<String> {
    let mut lines: Vec<String> = log
        .lines()
        .map(str::trim_end)
        .filter(|l| !l.trim().is_empty())
        .rev()
        .take(n)
        .map(str::to_owned)
        .collect();
    lines.reverse();
    lines
}

pub fn lines(code: u32, loader_log: &str) -> Vec<String> {
    let Some(sentence) = sentence(code) else {
        return Vec::new();
    };
    let mut out = vec![format!("{sentence}.")];
    let tail = last_lines(loader_log, LOADER_TAIL_LINES);
    if tail.is_empty() {
        out.push("The session's loader.log has no lines.".into());
    } else {
        out.push("Last loader.log lines:".into());
        out.extend(tail.into_iter().map(|l| format!("  {l}")));
    }
    out
}

pub fn session_lines(session: &Path) -> Vec<String> {
    let record = crate::session::SessionRecord::read(session);
    let Some(code) = record.end.as_ref().and_then(|e| e.exit_code).or(record.exit_code) else {
        return Vec::new();
    };
    let log = std::fs::read(session.join("loader.log")).unwrap_or_default();
    let tail = &log[log.len().saturating_sub(64 << 10)..];
    lines(code as u32, &String::from_utf8_lossy(tail))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_exit_codes() {
        assert_eq!(classify(0), ExitClass::Clean);
        assert_eq!(classify(0xc000_0409_u32 as i32), ExitClass::FastFail);
        assert_eq!(classify(0xc000_0602_u32 as i32), ExitClass::FastFail);
        assert_eq!(classify(0xc000_0417_u32 as i32), ExitClass::FastFail);
        assert_eq!(classify(0xc000_0005_u32 as i32), ExitClass::Crash);
        assert_eq!(classify(-1), ExitClass::Other);
        assert_eq!(classify(3), ExitClass::Other);
    }

    #[test]
    fn names_the_stack_buffer_overrun_in_one_sentence() {
        assert_eq!(
            sentence(0xc000_0409).as_deref(),
            Some("F1 25 stopped itself after detecting a corrupted stack or an invalid call (0xc0000409)")
        );
        assert_eq!(sentence(0xc000_0005), None);
    }

    #[test]
    fn keeps_the_last_loader_lines_without_blanks() {
        let log = "a\n\nb\r\nc\n\nd\n";
        assert_eq!(last_lines(log, 2), vec!["c", "d"]);
        assert_eq!(last_lines(log, 10), vec!["a", "b", "c", "d"]);
        assert!(last_lines("\n\n", 3).is_empty());
    }

    #[test]
    fn the_notice_is_the_sentence_then_the_log_tail() {
        let out = lines(0xc000_0409, "one\ntwo\nthree\n");
        assert_eq!(
            out[0],
            "F1 25 stopped itself after detecting a corrupted stack or an invalid call (0xc0000409)."
        );
        assert_eq!(out[1], "Last loader.log lines:");
        assert_eq!(out[2..], ["  one", "  two", "  three"]);
        assert!(lines(0xc000_0005, "x").is_empty());
        assert_eq!(
            lines(0xc000_0409, "")[1],
            "The session's loader.log has no lines."
        );
    }

    #[test]
    fn reads_a_session_folder() {
        let dir = std::env::temp_dir().join(format!("swofty-fast-fail-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(session_lines(&dir).is_empty());
        std::fs::write(dir.join("exit_code"), "-1073740791\n").unwrap();
        std::fs::write(dir.join("loader.log"), "[core] up\n[crash] armed\n").unwrap();
        let out = session_lines(&dir);
        assert!(out[0].ends_with("(0xc0000409)."), "{out:?}");
        assert_eq!(out.last().unwrap(), "  [crash] armed");
        std::fs::remove_dir_all(&dir).ok();
    }
}
