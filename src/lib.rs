//! The SDK for Litus plugins.
//!
//! A plugin is a WebAssembly module with a `plugin.json` manifest. It handles each request Litus
//! sends it (a command, a tool, or a parameter's list of choices) in one function, and reaches
//! Litus through [`host`]:
//!
//! ```no_run
//! use litus_plugin::{Kind, Request, ToolOutput, Value, geojson, host, json};
//!
//! fn handle(req: Request) -> Result<Value, String> {
//!     match (req.kind, req.id.as_str()) {
//!         (Kind::Command, "layer-report") => {
//!             let n = host::layers()?.as_array().map_or(0, Vec::len);
//!             host::notify(&format!("{n} layers"));
//!             Ok(json!({"layers": n}))
//!         }
//!         (Kind::Tool, "bounding-boxes") => {
//!             let boxes: Vec<Value> = req.features("input")?.iter().filter_map(|f| {
//!                 let b = geojson::bbox(&geojson::positions(&f["geometry"]))?;
//!                 Some(geojson::feature(geojson::polygon(&[[b[0], b[1]], [b[2], b[1]], [b[2], b[3]], [b[0], b[3]]]), json!({})))
//!             }).collect();
//!             Ok(ToolOutput::new().layer("Boxes", geojson::collection(boxes)).into())
//!         }
//!         (_, other) => Err(format!("unknown '{other}'")),
//!     }
//! }
//!
//! litus_plugin::export!(handle);
//! ```
//!
//! Build it with `cargo build --release --target wasm32-unknown-unknown`, as a `cdylib`. Each
//! call gets a fresh instance of the module, so nothing is kept between calls.
//!
//! Off WebAssembly (in a plugin's own unit tests), [`host`] answers through
//! [`host::set_mock`], and a handler can be called directly with [`Request::from_value`].

mod abi;
pub mod geojson;
pub mod host;
mod request;
mod tool;
mod translate;

pub use request::{File, Kind, Request};
pub use serde_json::{self, Map, Value, json};
pub use tool::ToolOutput;

/// The version of the plugin ABI this SDK speaks (`abi` in `plugin.json`).
pub const ABI: u32 = 1;

/// Exports the module's entry points for a handler `fn(Request) -> Result<Value, String>`.
///
/// Give the manifest too, `export!(handle, include_str!("../plugin.json"))`, for
/// [`Request::tr`] to translate messages with its `i18n` table.
#[macro_export]
macro_rules! export {
    ($handler:path) => {
        $crate::export!($handler, "");
    };
    ($handler:path, $manifest:expr) => {
        /// Room for `len` bytes, where Litus writes a request.
        #[unsafe(no_mangle)]
        pub extern "C" fn litus_alloc(len: i32) -> i32 {
            $crate::__private::alloc(len)
        }

        /// Handles the request at `ptr`; returns where its reply is (`ptr << 32 | len`).
        #[unsafe(no_mangle)]
        pub extern "C" fn litus_call(ptr: i32, len: i32) -> i64 {
            $crate::__private::call(ptr, len, $manifest, $handler)
        }
    };
}

/// What [`export!`] expands to. Not part of the API.
#[doc(hidden)]
pub mod __private {
    pub use crate::abi::{alloc, call};
}
