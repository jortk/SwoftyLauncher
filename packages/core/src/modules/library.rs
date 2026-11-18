//! Loads a library module and talks to it through the module C ABI, version 1: JSON requests in,
//! JSON answers out, with the module able to ask the launcher things and to announce events.

use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

use serde_json::{Value, json};

use super::Module;

/// The table a module gets from the launcher. Layout shared with the module; never reorder.
#[repr(C)]
struct SwoftyModuleHost {
    size: u32,
    version: u32,
    user: *mut c_void,
    log: unsafe extern "C" fn(user: *mut c_void, level: u32, msg: *const u8, len: usize),
    call: unsafe extern "C" fn(
        user: *mut c_void,
        req: *const u8,
        len: usize,
        out: *mut *mut u8,
        out_len: *mut usize,
    ) -> i32,
    free: unsafe extern "C" fn(user: *mut c_void, ptr: *mut u8, len: usize),
    notify: unsafe extern "C" fn(user: *mut c_void, json: *const u8, len: usize),
}

type AbiFn = unsafe extern "C" fn() -> u32;
type OpenFn = unsafe extern "C" fn(*const SwoftyModuleHost, *const u8, usize) -> *mut c_void;
type CallFn = unsafe extern "C" fn(*mut c_void, *const u8, usize, *mut *mut u8, *mut usize) -> i32;
type FreeFn = unsafe extern "C" fn(*mut u8, usize);
type CloseFn = unsafe extern "C" fn(*mut c_void);

/// Answers a request from the module: `{"op": ...}` in, the value or a sentence out.
pub type HostCall = dyn Fn(&Value) -> Result<Value, String> + Send + Sync;
/// Receives a module event, `{"event": ...}`; may run on any thread.
pub type Notify = dyn Fn(&Value) + Send + Sync;

/// What the launcher offers a module.
pub struct Hooks {
    pub call: Box<HostCall>,
    pub notify: Box<Notify>,
}

impl Default for Hooks {
    fn default() -> Self {
        Hooks {
            call: Box::new(|request| {
                Err(format!(
                    "the launcher has no answer for `{}`",
                    request.get("op").and_then(Value::as_str).unwrap_or("?")
                ))
            }),
            notify: Box::new(|_| {}),
        }
    }
}

struct HostState {
    name: String,
    hooks: Hooks,
}

/// An opened module. Closed when dropped.
pub struct Library {
    handle: *mut c_void,
    call: CallFn,
    free: FreeFn,
    close: CloseFn,
    // Dropped after `close` ran: the module may call the host until then.
    _table: Box<SwoftyModuleHost>,
    _state: Box<HostState>,
    _lib: libloading::Library,
}

// SAFETY: the ABI lets `swofty_module_call` run from any thread, and the host state is Send + Sync.
unsafe impl Send for Library {}
// SAFETY: as above.
unsafe impl Sync for Library {}

unsafe extern "C" fn host_log(user: *mut c_void, level: u32, msg: *const u8, len: usize) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: `user` is the `HostState` this library owns; `msg` is `len` readable bytes.
        let (state, text) = unsafe { (&*user.cast::<HostState>(), std::slice::from_raw_parts(msg, len)) };
        let level = ["info", "warning", "error"]
            .get(level as usize)
            .unwrap_or(&"info");
        eprintln!(
            "module {}: {level}: {}",
            state.name,
            String::from_utf8_lossy(text)
        );
    }));
}

unsafe extern "C" fn host_call(
    user: *mut c_void,
    req: *const u8,
    len: usize,
    out: *mut *mut u8,
    out_len: *mut usize,
) -> i32 {
    if out.is_null() || out_len.is_null() {
        return -1;
    }
    let answered = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: `user` is the `HostState` this library owns; `req` is `len` readable bytes.
        let (state, bytes) = unsafe { (&*user.cast::<HostState>(), std::slice::from_raw_parts(req, len)) };
        match serde_json::from_slice::<Value>(bytes) {
            Ok(request) => (state.hooks.call)(&request),
            Err(e) => Err(format!("the request is not JSON: {e}")),
        }
    }));
    let (code, doc) = match answered {
        Ok(Ok(v)) => (0, json!({ "ok": v })),
        Ok(Err(e)) => (1, json!({ "error": e })),
        Err(_) => (2, json!({ "error": "the launcher hit an internal error" })),
    };
    let boxed = doc.to_string().into_bytes().into_boxed_slice();
    // SAFETY: both pointers were checked and are the module's to receive the answer in.
    unsafe {
        *out_len = boxed.len();
        *out = Box::into_raw(boxed).cast::<u8>();
    }
    code
}

unsafe extern "C" fn host_free(_user: *mut c_void, ptr: *mut u8, len: usize) {
    if !ptr.is_null() {
        // SAFETY: `host_call` made this box with exactly this length.
        drop(unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr, len)) });
    }
}

unsafe extern "C" fn host_notify(user: *mut c_void, json: *const u8, len: usize) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: `user` is the `HostState` this library owns; `json` is `len` readable bytes.
        let (state, bytes) = unsafe { (&*user.cast::<HostState>(), std::slice::from_raw_parts(json, len)) };
        if let Ok(event) = serde_json::from_slice::<Value>(bytes) {
            (state.hooks.notify)(&event);
        }
    }));
}

impl Library {
    /// Opens the verified `module`, which must speak `abi`, with `config` as its open document.
    pub fn open(module: &Module, abi: u32, config: &Value, hooks: Hooks) -> Result<Library, String> {
        Self::open_path(&module.name, &module.path, abi, config, hooks)
    }

    /// [`Library::open`] for a file; `name` is for log lines.
    pub fn open_path(
        name: &str,
        path: &Path,
        abi: u32,
        config: &Value,
        hooks: Hooks,
    ) -> Result<Library, String> {
        let failed = |what: &str, e: &dyn std::fmt::Display| format!("module {name}: {what}: {e}");
        // SAFETY: the file is a module the caller verified (or a developer's own build); loading it runs its
        // initialisers, as loading any library does.
        let lib = unsafe { libloading::Library::new(path) }.map_err(|e| failed("couldn't be loaded", &e))?;
        // SAFETY: the symbol types are the module ABI's.
        let (abi_fn, open, call, free, close) = unsafe {
            (
                *lib.get::<AbiFn>(b"swofty_module_abi\0")
                    .map_err(|e| failed("is not a module", &e))?,
                *lib.get::<OpenFn>(b"swofty_module_open\0")
                    .map_err(|e| failed("is not a module", &e))?,
                *lib.get::<CallFn>(b"swofty_module_call\0")
                    .map_err(|e| failed("is not a module", &e))?,
                *lib.get::<FreeFn>(b"swofty_module_free\0")
                    .map_err(|e| failed("is not a module", &e))?,
                *lib.get::<CloseFn>(b"swofty_module_close\0")
                    .map_err(|e| failed("is not a module", &e))?,
            )
        };
        // SAFETY: takes nothing and returns a number.
        let speaks = unsafe { abi_fn() };
        if speaks != abi {
            return Err(format!(
                "module {name}: speaks abi {speaks}, this launcher speaks {abi}"
            ));
        }
        let mut state = Box::new(HostState {
            name: name.to_owned(),
            hooks,
        });
        let table = Box::new(SwoftyModuleHost {
            size: u32::try_from(std::mem::size_of::<SwoftyModuleHost>()).unwrap_or(0),
            version: 1,
            user: std::ptr::from_mut::<HostState>(state.as_mut()).cast(),
            log: host_log,
            call: host_call,
            free: host_free,
            notify: host_notify,
        });
        let config = config.to_string();
        // SAFETY: the table and the state live in boxes this `Library` keeps until after `close`.
        let handle = unsafe { open(std::ptr::from_ref(table.as_ref()), config.as_ptr(), config.len()) };
        if handle.is_null() {
            return Err(format!("module {name}: didn't open"));
        }
        Ok(Library {
            handle,
            call,
            free,
            close,
            _table: table,
            _state: state,
            _lib: lib,
        })
    }

    /// Sends `{"op": op, ...args}`; `args` is an object or null. The module's value, or its sentence.
    pub fn call(&self, op: &str, args: Value) -> Result<Value, String> {
        let mut request = match args {
            Value::Object(map) => Value::Object(map),
            Value::Null => json!({}),
            other => return Err(format!("the arguments of `{op}` are not an object: {other}")),
        };
        request["op"] = json!(op);
        let text = request.to_string();
        let mut out: *mut u8 = std::ptr::null_mut();
        let mut out_len = 0usize;
        // SAFETY: an open handle, a live request buffer and two writable out slots.
        let code = unsafe { (self.call)(self.handle, text.as_ptr(), text.len(), &mut out, &mut out_len) };
        if out.is_null() {
            return Err(format!("`{op}` got no answer (code {code})"));
        }
        // SAFETY: the module returned `out_len` readable bytes at `out`.
        let bytes = unsafe { std::slice::from_raw_parts(out, out_len) }.to_vec();
        // SAFETY: the buffer came from this module's `swofty_module_call`.
        unsafe { (self.free)(out, out_len) };
        let mut doc: Value =
            serde_json::from_slice(&bytes).map_err(|e| format!("the answer to `{op}` is not JSON: {e}"))?;
        if let Some(e) = doc.get("error") {
            return Err(e.as_str().map_or_else(|| e.to_string(), str::to_owned));
        }
        Ok(doc.get_mut("ok").map(Value::take).unwrap_or(Value::Null))
    }
}

impl Drop for Library {
    fn drop(&mut self) {
        // SAFETY: the handle is open and closed once, here.
        unsafe { (self.close)(self.handle) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs against a locally built library module when `SWOFTY_MODULE_TEST_LIBRARY` names one.
    #[test]
    fn a_local_library_module_opens_answers_and_refuses_unknown_ops() {
        let Some(path) = std::env::var_os("SWOFTY_MODULE_TEST_LIBRARY") else {
            return;
        };
        let config = json!({ "data": std::env::temp_dir().join("swofty-module-library-test"), "server": "" });
        let wrong = Library::open_path("local", Path::new(&path), 0, &config, Hooks::default());
        assert!(wrong.err().is_some_and(|e| e.contains("abi")));
        let lib = Library::open_path("local", Path::new(&path), 1, &config, Hooks::default()).unwrap();
        assert_eq!(lib.call("version", Value::Null).unwrap()["abi"], 1);
        assert!(lib.call("no.such.op", json!({})).is_err());
        assert!(lib.call("version", json!(3)).is_err());
    }

    #[test]
    fn a_file_that_is_not_a_library_is_an_error() {
        let path = std::env::temp_dir().join(format!("swofty-not-a-module-{}", std::process::id()));
        std::fs::write(&path, b"nope").unwrap();
        assert!(Library::open_path("x", &path, 1, &json!({}), Hooks::default()).is_err());
        std::fs::remove_file(&path).ok();
    }
}
