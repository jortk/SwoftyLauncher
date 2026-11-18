//! The `swofty offline` commands and the launch gate for offline play.
//!
//! See `swofty_core::offline` for the model: consent, digest-pinned extension
//! artifacts, and journaled staging that returns the game folder to vanilla.

use std::io::{IsTerminal, Write};
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::Subcommand;
use swofty_core::offline::{self, Artifacts};
use swofty_core::store::Store;

#[derive(Subcommand)]
pub enum OfflineCommand {
    #[command(about = "Show consent state, the extension's digests and where it came from")]
    Status,
    #[command(about = "Verify and install the extension from a downloaded release folder")]
    Install { dir: PathBuf },
    #[command(about = "Download the pinned release and verify its digests")]
    Fetch,
    #[command(about = "Read the offline play terms and accept them")]
    Accept,
    #[command(about = "Withdraw consent; modded launches will be refused")]
    Withdraw,
}

pub fn command(store: &Store, cmd: OfflineCommand) -> Result<()> {
    match cmd {
        OfflineCommand::Status => status(store),
        OfflineCommand::Install { dir } => {
            let a = offline::install(&store.root, &dir)
                .with_context(|| format!("installing from {}", dir.display()))?;
            println!(
                "installed offline play {} from {}",
                offline::EXTENSION_VERSION,
                dir.display()
            );
            println!("  {}", a.dir.display());
            Ok(())
        }
        OfflineCommand::Fetch => {
            let a = fetch(store)?;
            println!(
                "installed offline play {} into {}",
                offline::EXTENSION_VERSION,
                a.dir.display()
            );
            Ok(())
        }
        OfflineCommand::Accept => {
            prompt_accept(store)?;
            Ok(())
        }
        OfflineCommand::Withdraw => {
            offline::withdraw(&store.root)?;
            println!("consent withdrawn: modded launches will be refused");
            println!(
                "the extension files stay in {}; remove them with your file manager if you want",
                offline::extension_dir(&store.root).display()
            );
            Ok(())
        }
    }
}

fn status(store: &Store) -> Result<()> {
    let accepted = offline::is_accepted(&store.root);
    println!(
        "consent:        {}",
        if accepted { "accepted" } else { "not accepted" }
    );
    if let (true, Some(c)) = (accepted, offline::consent(&store.root)) {
        println!("accepted at:    unix {}", c.accepted_utc);
    }
    println!("terms:          swofty offline accept");
    match offline::artifacts(&store.root) {
        Ok(a) => {
            println!(
                "extension:      {} (v{})",
                a.dir.display(),
                offline::EXTENSION_VERSION
            );
            println!("source:         {}", offline::SOURCE_URL);
            match offline::exec_mode(&a.probe).and_then(|_| offline::exec_mode(&a.prelaunch)) {
                Ok(_) => println!(
                    "runs:           {}",
                    loader_note(&a).unwrap_or_else(|| "directly".into())
                ),
                Err(e) => println!("runs:           no: {e}"),
            }
        }
        Err(e) => {
            println!("extension:      not usable: {e}");
            println!(
                "host:           {}/v{}",
                offline::base_url(),
                offline::EXTENSION_VERSION
            );
            println!("install with:   swofty offline fetch, or swofty offline install <downloaded folder>");
        }
    }
    let journal = offline::journal_path(&store.root);
    println!(
        "staging:        {}",
        if journal.exists() {
            "pending (run swofty restore)"
        } else {
            "clean"
        }
    );
    Ok(())
}

/// Download the pinned release, telling the user where it comes from.
pub fn fetch(store: &Store) -> Result<Artifacts> {
    println!(
        "downloading offline play {} from {} (verified against pinned SHA-256 digests)",
        offline::EXTENSION_VERSION,
        offline::base_url()
    );
    let a = offline::fetch(&store.root).context("downloading the offline play extension")?;
    Ok(a)
}

/// Say so when the tools start through the system loader instead of the one
/// they were built against.
pub fn loader_note(a: &Artifacts) -> Option<String> {
    match offline::exec_mode(&a.probe) {
        Ok(offline::Exec::Loader { wanted, system }) => Some(format!(
            "the extension's Linux tools were built for {}, which this system does not have; \
             running them through {}",
            wanted.display(),
            system.display()
        )),
        _ => None,
    }
}

/// The gate: every modded launch passes through here. The terms are shown
/// once; declining (or not answering) means the game is not launched.
pub fn gate(store: &Store, interactive: bool) -> Result<()> {
    if offline::is_accepted(&store.root) {
        return Ok(());
    }
    if !interactive {
        println!("{}", offline::DISCLOSURE);
        bail!("offline play terms not accepted: F1 25 was not launched (run `swofty offline accept`)");
    }
    prompt_accept(store)?;
    println!(
        "accepted: modded sessions run offline without EA AntiCheat (withdraw: `swofty offline withdraw`)"
    );
    Ok(())
}

/// Show the terms and ask for a typed acceptance.
pub fn prompt_accept(store: &Store) -> Result<()> {
    if !offline::is_accepted(&store.root) {
        println!("{}", offline::DISCLOSURE);
    }
    if !std::io::stdin().is_terminal() {
        bail!("stdin is not a terminal: run `swofty offline accept` interactively");
    }
    print!("\nType 'accept' to continue: ");
    std::io::stdout().flush().ok();
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    if line.trim() != "accept" {
        bail!("declined: nothing was changed");
    }
    offline::record_accept(&store.root)?;
    Ok(())
}

/// Make sure a verified extension is present before a modded session.
pub fn ensure_extension(store: &Store) -> Result<Artifacts> {
    match offline::artifacts(&store.root) {
        Ok(a) => Ok(a),
        Err(e) => {
            println!("{}", why_fetch(store, &e));
            fetch(store)
        }
    }
}

fn why_fetch(store: &Store, e: &swofty_core::Error) -> String {
    if let Some(stub) = offline::stub_removed_from_extension(&store.root) {
        return format!(
            "offline play: {}; fetching the pinned release",
            offline::stub_removed_text(&store.root, &stub)
        );
    }
    if offline::extension_dir(&store.root).exists() {
        // A copy that fails its pins is stale or tampered. The pins
        // are the trust decision, so the pinned release replaces it.
        return format!("offline play extension does not match the pins ({e}); fetching the pinned release");
    }
    format!("offline play extension is not installed yet: {e}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_download_leads_with_the_reason_and_offers_no_manual_download() {
        let root = std::env::temp_dir().join(format!("swofty-cli-offline-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        // SAFETY: std serialises its own environment access, and nothing in this
        // binary reads the environment from C.
        unsafe { std::env::set_var("SWOFTY_OFFLINE_PLAY_BASE_URL", "http://127.0.0.1:1") };
        let err = format!("{:#}", ensure_extension(&Store::new(&root)).unwrap_err());
        let (reason, detail) = err.split_once('\n').unwrap();
        assert!(
            reason.starts_with(&format!(
                "downloading the offline play extension: offline play {}: \
                 the download was blocked on this PC or network",
                offline::EXTENSION_VERSION
            )),
            "{err}"
        );
        assert!(
            detail.starts_with("(could not download http://127.0.0.1:1/"),
            "{err}"
        );
        assert!(
            !err.contains("yourself") && !err.contains("offline install"),
            "{err}"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_stub_gone_beside_its_hook_is_put_down_to_an_antivirus() {
        let root = std::env::temp_dir().join(format!("swofty-cli-offline-av-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let store = Store::new(&root);
        let dir = offline::extension_dir(&root);
        std::fs::create_dir_all(&dir).unwrap();
        for name in offline::REQUIRED {
            std::fs::write(dir.join(name), "x").unwrap();
        }
        std::fs::remove_file(dir.join("f1op_stub.exe")).unwrap();
        let e = offline::artifacts(&root).unwrap_err();
        let line = why_fetch(&store, &e);
        assert!(
            line.starts_with("offline play: f1op_stub.exe is missing from ")
                && line.contains("an antivirus most likely removed it")
                && line.contains("quarantine")
                && line.contains(&format!(
                    "exclusion for the Swofty data folder ({})",
                    root.display()
                ))
                && line.ends_with("; fetching the pinned release"),
            "{line}"
        );
        std::fs::remove_file(dir.join("f1op_hook.dll")).unwrap();
        let e = offline::artifacts(&root).unwrap_err();
        assert!(
            why_fetch(&store, &e).starts_with("offline play extension does not match the pins"),
            "{}",
            why_fetch(&store, &e)
        );
        std::fs::remove_dir_all(&root).ok();
    }
}
