//! `swofty-ads --parent <x11 window> --data <dir>`: hosts the launcher's ad
//! slots on Linux as child windows of the launcher's X11 window. It reads
//! `swofty_ads::Command` lines on stdin and quits when stdin closes.

#[cfg(target_os = "linux")]
mod linux;

fn main() {
    #[cfg(target_os = "linux")]
    std::process::exit(linux::run());
    #[cfg(not(target_os = "linux"))]
    {
        eprintln!("swofty-ads only runs on Linux");
        std::process::exit(2);
    }
}
