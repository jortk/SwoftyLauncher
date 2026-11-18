//! Opens a library module file and sends it requests, to check a build of one by hand:
//! `module_probe <file> <abi> [<op> [<json arguments>]]...`. Each answer is printed on its own line.

use std::path::Path;
use std::process::ExitCode;

use serde_json::{Value, json};
use swofty_core::modules::library::{Hooks, Library};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(file), Some(abi)) = (args.first(), args.get(1).and_then(|a| a.parse::<u32>().ok())) else {
        eprintln!("usage: module_probe <file> <abi> [<op> [<json arguments>]]...");
        return ExitCode::from(2);
    };
    let config = json!({ "data": std::env::temp_dir().join("swofty-module-probe"), "server": "" });
    let hooks = Hooks {
        notify: Box::new(|event| println!("event {event}")),
        ..Hooks::default()
    };
    let library = match Library::open_path("probe", Path::new(file), abi, &config, hooks) {
        Ok(library) => library,
        Err(e) => {
            eprintln!("open failed: {e}");
            return ExitCode::FAILURE;
        }
    };
    println!("opened {file} at abi {abi}");
    let mut rest = args[2..].iter().peekable();
    while let Some(op) = rest.next() {
        let arguments = match rest.peek().and_then(|a| serde_json::from_str::<Value>(a).ok()) {
            Some(v) if v.is_object() => {
                rest.next();
                v
            }
            _ => Value::Null,
        };
        match library.call(op, arguments) {
            Ok(v) => println!("{op} ok {v}"),
            Err(e) => println!("{op} error {e}"),
        }
    }
    drop(library);
    println!("closed");
    ExitCode::SUCCESS
}
