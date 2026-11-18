//! Evidence for an EA app launch that started nothing: which EA-side
//! processes came and went after the launch link, and the Windows event log
//! entries and EA app logs around it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use crate::GAME_EXE;

pub const WATCHED: [&str; 4] = [
    GAME_EXE,
    "EAAntiCheat.GameServiceLauncher.exe",
    "EAAntiCheat.GameService.exe",
    "EAAntiCheat.Installer.Tool.exe",
];
pub const SHORT_LIVED: Duration = Duration::from_secs(30);
pub const EVENT_WINDOW_MS: u64 = 24 * 3_600_000;
pub const EVENTS_KEPT: usize = 40;
pub const EVENTS_READ: usize = 400;
pub const LOG_FILES: usize = 4;
pub const LOG_TAIL: u64 = 256 << 10;
const EVENT_MARKERS: [&str; 5] = [
    "f1_25.exe",
    "eaanticheat",
    "eadesktop",
    "ealauncher",
    "electronic arts",
];

pub fn describe_elevation(elevated: Option<bool>) -> &'static str {
    match elevated {
        Some(true) => "elevated (administrator)",
        Some(false) => "not elevated",
        None => "unknown (not readable; an elevated process is not readable from a normal one)",
    }
}

pub fn elevation_line(launcher: Option<bool>, ea_app: Option<Option<bool>>) -> String {
    let app = match ea_app {
        Some(e) => describe_elevation(e).to_owned(),
        None => "not running".to_owned(),
    };
    format!(
        "launcher is {}; EA app (EADesktop.exe) is {app}",
        describe_elevation(launcher)
    )
}

pub fn exit_text(code: Option<u32>) -> String {
    match code {
        Some(c) => format!("exit code {c} (0x{c:08x})"),
        None => "exit code unreadable".to_owned(),
    }
}

struct Seen {
    name: String,
    appeared: Instant,
    probe: Option<crate::process::ExitProbe>,
}

pub struct Tracker {
    sent: Instant,
    ignore: Vec<u32>,
    live: HashMap<u32, Seen>,
    history: Vec<String>,
}

impl Tracker {
    pub fn new(sent: Instant, running: &[(String, u32)]) -> Self {
        Self {
            sent,
            ignore: running.iter().map(|(_, pid)| *pid).collect(),
            live: HashMap::new(),
            history: Vec::new(),
        }
    }

    pub fn observe(&mut self, running: &[(String, u32)], now: Instant) -> Vec<String> {
        let mut lines = Vec::new();
        for (name, pid) in running {
            if self.ignore.contains(pid) || self.live.contains_key(pid) {
                continue;
            }
            lines.push(format!(
                "{name} (pid {pid}) appeared {} s after the launch link",
                now.duration_since(self.sent).as_secs()
            ));
            self.live.insert(
                *pid,
                Seen {
                    name: name.clone(),
                    appeared: now,
                    probe: crate::process::ExitProbe::open(*pid),
                },
            );
        }
        let gone: Vec<u32> = self
            .live
            .keys()
            .copied()
            .filter(|pid| !running.iter().any(|(_, p)| p == pid))
            .collect();
        for pid in gone {
            let Some(seen) = self.live.remove(&pid) else {
                continue;
            };
            let lived = now.duration_since(seen.appeared);
            let code = seen.probe.as_ref().and_then(|p| p.exit_code());
            let short = if lived <= SHORT_LIVED {
                ", it exited right away"
            } else {
                ""
            };
            lines.push(format!(
                "{} (pid {pid}) exited after {} s with {}{short}",
                seen.name,
                lived.as_secs(),
                exit_text(code)
            ));
        }
        self.history.extend(lines.iter().cloned());
        lines
    }

    pub fn summary(&self, waited: Duration) -> String {
        let seen = if self.history.is_empty() {
            "no F1_25.exe or EAAntiCheat process appeared".to_owned()
        } else {
            format!("saw: {}", self.history.join("; "))
        };
        let still: Vec<String> = self
            .live
            .iter()
            .map(|(pid, s)| format!("{} (pid {pid})", s.name))
            .collect();
        let still = if still.is_empty() {
            String::new()
        } else {
            format!("; still running: {}", still.join(", "))
        };
        format!(
            "no game after {} s of the launch link; {seen}{still}",
            waited.as_secs()
        )
    }
}

pub fn running_watched() -> Vec<(String, u32)> {
    WATCHED
        .iter()
        .flat_map(|name| {
            crate::process::named(name)
                .into_iter()
                .map(move |p| ((*name).to_owned(), p.pid))
        })
        .collect()
}

pub fn ea_app_elevation() -> Option<Option<bool>> {
    crate::process::named(crate::ea_app::PROCESS)
        .first()
        .map(|p| crate::process::elevation(p.pid))
}

pub fn event_query(window_ms: u64) -> String {
    format!("*[System[TimeCreated[timediff(@SystemTime) <= {window_ms}]]]")
}

pub fn events(text: &str) -> Vec<&str> {
    let mut starts: Vec<usize> = text.match_indices("Event[").map(|(i, _)| i).collect();
    starts.retain(|&i| i == 0 || text.as_bytes()[i - 1] == b'\n');
    starts
        .iter()
        .enumerate()
        .map(|(n, &s)| text[s..starts.get(n + 1).copied().unwrap_or(text.len())].trim_end())
        .collect()
}

pub fn is_relevant(event: &str) -> bool {
    let lower = event.to_ascii_lowercase();
    EVENT_MARKERS.iter().any(|m| lower.contains(m))
        && (lower.contains("application error")
            || lower.contains("windows error reporting")
            || lower.contains("application hang")
            || lower.contains("eaanticheat")
            || lower.contains("eadesktop")
            || lower.contains("f1_25.exe"))
}

pub fn relevant_events(text: &str, keep: usize) -> Vec<&str> {
    let all: Vec<&str> = events(text).into_iter().filter(|e| is_relevant(e)).collect();
    all[all.len().saturating_sub(keep)..].to_vec()
}

pub fn events_report(text: &str) -> String {
    let kept = relevant_events(text, EVENTS_KEPT);
    if kept.is_empty() {
        return "no Application log entries naming F1_25.exe, EAAntiCheat or the EA app in the last 24 hours\n"
            .to_owned();
    }
    let mut out = format!(
        "{} Application log entries from the last 24 hours that name F1_25.exe, EAAntiCheat or the EA app, newest first\n\n",
        kept.len()
    );
    for e in kept {
        out.push_str(e);
        out.push_str("\n\n");
    }
    out
}

#[cfg(windows)]
pub fn application_events() -> Result<String, String> {
    let query = format!("/q:{}", event_query(EVENT_WINDOW_MS));
    let count = format!("/c:{EVENTS_READ}");
    let raw = crate::compat::windows_tool_within(
        "wevtutil",
        &["qe", "Application", &query, "/f:text", &count, "/rd:true"],
        Duration::from_secs(20),
    )?;
    Ok(events_report(&raw))
}

#[cfg(not(windows))]
pub fn application_events() -> Result<String, String> {
    Err("the Windows event log is only on Windows".to_owned())
}

pub fn log_dirs() -> Vec<PathBuf> {
    if !cfg!(windows) {
        return Vec::new();
    }
    [
        std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join(r"Electronic Arts\EA Desktop\Logs")),
        std::env::var_os("ProgramData").map(|d| PathBuf::from(d).join(r"EA Desktop\Logs")),
    ]
    .into_iter()
    .flatten()
    .collect()
}

pub fn newest_logs(dirs: &[PathBuf], n: usize) -> Vec<PathBuf> {
    let mut found: Vec<(SystemTime, PathBuf)> = dirs
        .iter()
        .flat_map(|d| std::fs::read_dir(d).into_iter().flatten().flatten())
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| {
            let path = e.path();
            let ext = path.extension()?.to_str()?.to_ascii_lowercase();
            matches!(ext.as_str(), "log" | "txt").then_some(())?;
            Some((e.metadata().ok()?.modified().ok()?, path))
        })
        .collect();
    found.sort_by_key(|(t, _)| std::cmp::Reverse(*t));
    found.into_iter().take(n).map(|(_, p)| p).collect()
}

pub const PROXY_LOG: &str = "EAEgsProxy.log";
const VIRUS_TEXT: &str = "contains a virus or potentially unwanted software";
const VIRUS_CODE: &str = "0xe1";
const NOT_FOUND_TEXT: &str = "cannot find the file specified";
const NOT_FOUND_CODE: &str = " 0x2";

pub fn proxy_virus_block(log: &str, exe: &str, since: i64) -> bool {
    proxy_start_failed(log, exe, (VIRUS_TEXT, VIRUS_CODE), |t| {
        t.is_none_or(|t| t >= since)
    })
}

/// The proxy found no file to start at or after `staged`, when offline play
/// put its file there. Something removed that file within seconds, which is
/// an antivirus: an EA app repair writes EA's own file in its place. Only a
/// line with its time counts.
pub fn proxy_file_missing(log: &str, exe: &str, staged: i64) -> bool {
    proxy_start_failed(log, exe, (NOT_FOUND_TEXT, NOT_FOUND_CODE), |t| {
        t.is_some_and(|t| t >= staged)
    })
}

fn proxy_start_failed(
    log: &str,
    exe: &str,
    (text, code): (&str, &str),
    when: impl Fn(Option<i64>) -> bool,
) -> bool {
    let exe = exe.to_ascii_lowercase();
    log.lines().any(|line| {
        let lower = line.to_ascii_lowercase();
        lower.contains("createprocess")
            && lower.contains(&exe)
            && (lower.contains(text) || lower.trim_end().ends_with(code))
            && when(
                line.split_once('[')
                    .and_then(|(_, rest)| crate::catalog::epoch(rest)),
            )
    })
}

pub fn proxy_logs() -> Vec<String> {
    log_dirs()
        .iter()
        .map(|d| d.join(PROXY_LOG))
        .filter_map(|p| tail(&p, LOG_TAIL))
        .collect()
}

fn tail(path: &Path, cap: u64) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    f.seek(SeekFrom::Start(len.saturating_sub(cap))).ok()?;
    let mut data = Vec::new();
    f.read_to_end(&mut data).ok()?;
    Some(String::from_utf8_lossy(&data).into_owned())
}

pub fn log_entry_name(path: &Path, taken: &mut Vec<String>) -> String {
    let base = path
        .file_name()
        .map_or_else(|| "log".to_owned(), |n| n.to_string_lossy().into_owned());
    let mut name = base.clone();
    let mut n = 2;
    while taken.contains(&name) {
        name = format!("{n}-{base}");
        n += 1;
    }
    taken.push(name.clone());
    name
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "Event[0]:\n  Log Name: Application\n  Source: Application Error\n  Description:\n\
Faulting application name: F1_25.exe, version: 1.0\n\nEvent[1]:\n  Log Name: Application\n  Source: Chrome\n  \
Description: nothing\n\nEvent[2]:\n  Log Name: Application\n  Source: Windows Error Reporting\n  Description:\n\
Fault bucket, type 5\nP1: EADesktop.exe\n";

    const PROXY_BLOCKED: &str = "\
839:    20\t[2026-11-01T01:57:04.895Z]\tPID: 13588\tTID: 29620\tERROR   \t(eax::services::process::ProcessWin::createProcess)\t[PROCESS] Create Process: Operation did not complete successfully because the file contains a virus or potentially unwanted software. | exe=[C:\\F1 25\\F1 25\\EAAntiCheat.GameServiceLauncher.exe], args=[], dir=[C:\\F1 25\\F1 25\\] | OS ErrorCode:  0xe1
840:    21\t[2026-11-01T01:57:04.895Z]\tPID: 13588\tTID:  5128\tINFO    \t(eax::services::telemetry::TelemetryManagerV2::processEvent)\tTelemetry Event [prcs.create.erro]
";
    const EXE: &str = "EAAntiCheat.GameServiceLauncher.exe";

    #[test]
    fn finds_the_proxy_failing_to_start_the_stub() {
        assert!(proxy_virus_block(PROXY_BLOCKED, EXE, 0));
        let code_only = PROXY_BLOCKED.replace("Operation did not complete successfully because the file contains a virus or potentially unwanted software.", "failed");
        assert!(proxy_virus_block(&code_only, EXE, 0));
    }

    #[test]
    fn ignores_other_programs_other_failures_and_old_lines() {
        assert!(!proxy_virus_block(PROXY_BLOCKED, "EADesktop.exe", 0));
        assert!(!proxy_virus_block(
            "[2026-11-01T01:57:04.895Z] INFO createProcess EAAntiCheat.GameServiceLauncher.exe ok",
            EXE,
            0
        ));
        let after = crate::catalog::epoch("2026-11-01T02:00:00Z").unwrap();
        assert!(!proxy_virus_block(PROXY_BLOCKED, EXE, after));
    }

    const PROXY_MISSING: &str = "\
1255:    17\t[2026-11-13T10:00:00.698Z]\tPID: 14048\tTID: 19396\tERROR   \t(eax::services::process::ProcessWin::createProcess)\t[PROCESS] Create Process: The system cannot find the file specified. | exe=[C:\\Program Files\\EA Games\\F1 25\\EAAntiCheat.GameServiceLauncher.exe], args=[], dir=[C:\\Program Files\\EA Games\\F1 25\\], breakaway=[true], inheritEnv=[false], uac=[false] ut=[false] capture=[false], cwd=[C:\\Program Files\\EA Games\\F1 25] | OS ErrorCode:  0x2
1256:    18\t[2026-11-13T10:00:00.699Z]\tPID: 14048\tTID: 19396\tERROR   \t(eax::apps::ProxyComponent::launchProcess)\tProcess create error.
";
    const PROXY_STARTED: &str = "\
1140:    17\t[2026-11-13T09:51:51.791Z]\tPID: 25288\tTID: 10620\tINFO    \t(eax::services::process::ProcessWin::createProcess)\t[PROCESS] exe=[C:\\Program Files\\EA Games\\F1 25\\EAAntiCheat.GameServiceLauncher.exe], args=[], dir=[C:\\Program Files\\EA Games\\F1 25\\], breakaway=[true], inheritEnv=[false], uac=[false] ut=[false] capture=[false], cwd=[C:\\Program Files\\EA Games\\F1 25] | PID: 1264 |
";

    #[test]
    fn finds_the_proxy_missing_the_stub_after_staging() {
        let staged = crate::catalog::epoch("2026-11-13T09:59:49Z").unwrap();
        assert!(proxy_file_missing(PROXY_MISSING, EXE, staged));
        assert!(!proxy_virus_block(PROXY_MISSING, EXE, 0));
        let code_only = PROXY_MISSING.replace("The system cannot find the file specified.", "failed");
        assert!(proxy_file_missing(&code_only, EXE, staged));
    }

    #[test]
    fn a_missing_file_before_staging_or_a_started_launcher_is_not_a_removal() {
        let later = crate::catalog::epoch("2026-11-13T10:01:00Z").unwrap();
        assert!(!proxy_file_missing(PROXY_MISSING, EXE, later));
        assert!(!proxy_file_missing(PROXY_MISSING, "EADesktop.exe", 0));
        assert!(!proxy_file_missing(PROXY_STARTED, EXE, 0));
        assert!(!proxy_file_missing(PROXY_BLOCKED, EXE, 0));
        let undated = PROXY_MISSING.replace("[2026-11-13T10:00:00.698Z]", "");
        assert!(!proxy_file_missing(&undated, EXE, 0));
    }

    #[test]
    fn splits_events() {
        assert_eq!(events(SAMPLE).len(), 3);
        assert!(events(SAMPLE)[1].contains("Chrome"));
    }

    #[test]
    fn keeps_only_relevant_events() {
        let kept = relevant_events(SAMPLE, 10);
        assert_eq!(kept.len(), 2);
        assert!(kept[0].contains("F1_25.exe"));
        assert!(kept[1].contains("EADesktop.exe"));
        assert!(!is_relevant(
            "Source: Application Error\nFaulting application name: chrome.exe"
        ));
    }

    #[test]
    fn caps_the_events_kept_to_the_last() {
        let kept = relevant_events(SAMPLE, 1);
        assert_eq!(kept.len(), 1);
        assert!(kept[0].contains("EADesktop.exe"));
    }

    #[test]
    fn reports_when_nothing_matches() {
        assert!(events_report("Event[0]:\n Source: Chrome\n").starts_with("no Application log entries"));
        assert!(events_report(SAMPLE).starts_with("2 Application log entries"));
    }

    #[test]
    fn builds_the_time_query() {
        assert_eq!(
            event_query(86_400_000),
            "*[System[TimeCreated[timediff(@SystemTime) <= 86400000]]]"
        );
    }

    #[test]
    fn describes_elevation() {
        assert_eq!(
            elevation_line(Some(false), Some(Some(true))),
            "launcher is not elevated; EA app (EADesktop.exe) is elevated (administrator)"
        );
        assert!(elevation_line(Some(true), None).ends_with("is not running"));
        assert!(elevation_line(None, Some(None)).contains("unknown"));
    }

    #[test]
    fn formats_exit_codes() {
        assert_eq!(exit_text(Some(3)), "exit code 3 (0x00000003)");
        assert_eq!(exit_text(None), "exit code unreadable");
    }

    #[test]
    fn tracks_processes_that_come_and_go() {
        let t0 = Instant::now();
        let before = [("F1_25.exe".to_owned(), 7)];
        let mut t = Tracker::new(t0, &before);
        assert!(t.observe(&before, t0).is_empty());
        let eac = [
            before[0].clone(),
            ("EAAntiCheat.GameServiceLauncher.exe".to_owned(), 9),
        ];
        let at = t0 + Duration::from_secs(4);
        let lines = t.observe(&eac, at);
        assert_eq!(
            lines,
            ["EAAntiCheat.GameServiceLauncher.exe (pid 9) appeared 4 s after the launch link"]
        );
        assert!(t.observe(&eac, at).is_empty());
        let lines = t.observe(&before, t0 + Duration::from_secs(6));
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("exited after 2 s") && lines[0].ends_with("it exited right away"));
        let summary = t.summary(Duration::from_secs(300));
        assert!(summary.starts_with("no game after 300 s of the launch link; saw: "));
        assert!(summary.contains("pid 9"));
    }

    #[test]
    fn summary_says_when_nothing_appeared() {
        let t = Tracker::new(Instant::now(), &[]);
        assert_eq!(
            t.summary(Duration::from_secs(5)),
            "no game after 5 s of the launch link; no F1_25.exe or EAAntiCheat process appeared"
        );
    }

    #[test]
    fn picks_newest_logs_across_folders() {
        let root = std::env::temp_dir().join(format!("swofty-ealogs-{}", std::process::id()));
        let (a, b) = (root.join("a"), root.join("b"));
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        std::fs::write(a.join("old.log"), "x").unwrap();
        std::thread::sleep(Duration::from_millis(30));
        std::fs::write(b.join("new.log"), "x").unwrap();
        std::fs::write(b.join("skip.dll"), "x").unwrap();
        let found = newest_logs(&[a, b], 1);
        assert_eq!(found.len(), 1);
        assert!(found[0].ends_with("new.log"));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn names_duplicate_logs_apart() {
        let mut taken = Vec::new();
        assert_eq!(
            log_entry_name(Path::new("/a/EADesktop.log"), &mut taken),
            "EADesktop.log"
        );
        assert_eq!(
            log_entry_name(Path::new("/b/EADesktop.log"), &mut taken),
            "2-EADesktop.log"
        );
    }
}
