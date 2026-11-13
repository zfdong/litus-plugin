//! The ABI (version 1): the module's exports, and Litus's three imports in module `litus`.
//!
//! - `litus_alloc(len) -> ptr` gives Litus room for a request; `litus_call(ptr, len)` handles
//!   it and returns `reply_ptr << 32 | reply_len`, the reply being `{"ok": value}` or
//!   `{"error": "text"}`.
//! - `call(ptr, len) -> n` runs a host request (`{"op", "params"}`) and returns its reply's
//!   length (or -1); `result(ptr)` copies that reply to `ptr`; `log(ptr, len)` logs a line.
//!
//! Each call gets a fresh instance, so what is allocated here is never freed: the instance
//! goes away.

#![allow(unsafe_code)]

use serde_json::{Value, json};

use crate::Request;

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "litus")]
unsafe extern "C" {
    #[link_name = "call"]
    fn litus_host_call(ptr: *const u8, len: usize) -> i32;
    #[link_name = "result"]
    fn litus_host_result(ptr: *mut u8);
    #[link_name = "log"]
    fn litus_host_log(ptr: *const u8, len: usize);
}

/// Room for `len` bytes, left allocated.
pub fn alloc(len: i32) -> i32 {
    let mut room = Vec::<u8>::with_capacity(usize::try_from(len).unwrap_or(0));
    let ptr = room.as_mut_ptr();
    std::mem::forget(room);
    ptr as usize as i32
}

/// Reads the request at `ptr`, runs `handler` on it and returns where the reply is.
pub fn call(ptr: i32, len: i32, manifest: &'static str, handler: fn(Request) -> Result<Value, String>) -> i64 {
    install_panic_hook();
    crate::translate::set_manifest(manifest);
    let len = usize::try_from(len).unwrap_or(0);
    // SAFETY: Litus wrote `len` bytes at `ptr`, which `alloc(len)` gave it.
    let bytes = unsafe { std::slice::from_raw_parts(ptr as u32 as usize as *const u8, len) };
    let reply = match serde_json::from_slice::<Value>(bytes).map_err(|e| format!("unreadable request: {e}")).and_then(Request::from_value) {
        Ok(req) => match handler(req) {
            Ok(v) => json!({"ok": v}),
            Err(e) => json!({"error": e}),
        },
        Err(e) => json!({"error": e}),
    };
    let out: &'static [u8] = Box::leak(reply.to_string().into_bytes().into_boxed_slice());
    ((out.as_ptr() as usize as u64) << 32 | out.len() as u64) as i64
}

/// A panic traps the module, and Litus reports only that it stopped: log the message first.
fn install_panic_hook() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| std::panic::set_hook(Box::new(|info| log(&format!("the plugin panicked: {info}")))));
}

/// Sends `{"op", "params"}` to Litus and returns its answer.
#[cfg(target_arch = "wasm32")]
pub fn host_call(op: &str, params: &Value) -> Result<Value, String> {
    let req = json!({"op": op, "params": params}).to_string();
    // SAFETY: Litus reads `req.len()` bytes at its pointer, which stay alive for the call.
    let n = unsafe { litus_host_call(req.as_ptr(), req.len()) };
    let n = usize::try_from(n).map_err(|_| format!("{op}: Litus gave no answer"))?;
    let mut buf = vec![0u8; n];
    // SAFETY: Litus writes the `n` bytes it announced, and `buf` holds `n`.
    unsafe { litus_host_result(buf.as_mut_ptr()) };
    let reply: Value = serde_json::from_slice(&buf).map_err(|e| format!("{op}: unreadable answer from Litus: {e}"))?;
    reply_value(reply)
}

/// Off WebAssembly the mock answers (see [`crate::host::set_mock`]).
#[cfg(not(target_arch = "wasm32"))]
pub fn host_call(op: &str, params: &Value) -> Result<Value, String> {
    crate::host::mock_call(op, params)
}

/// `{"ok": value}` or `{"error": "text"}` as a result.
#[cfg(target_arch = "wasm32")]
fn reply_value(mut reply: Value) -> Result<Value, String> {
    if let Some(e) = reply.get("error") {
        return Err(e.as_str().map_or_else(|| e.to_string(), str::to_string));
    }
    Ok(reply.get_mut("ok").map(Value::take).unwrap_or(Value::Null))
}

/// Logs a line.
pub fn log(text: &str) {
    #[cfg(target_arch = "wasm32")]
    // SAFETY: Litus reads `text.len()` bytes at its pointer, which stay alive for the call.
    unsafe {
        litus_host_log(text.as_ptr(), text.len());
    }
    #[cfg(not(target_arch = "wasm32"))]
    eprintln!("{text}");
}
