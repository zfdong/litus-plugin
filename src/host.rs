//! Requests to Litus from a command (tools make none). Each one is checked against the
//! plugin's permissions; see "What a plugin can ask for" in the guide (`docs/guide.md`).
//!
//! Off WebAssembly, in a plugin's own tests, requests go to the function given to
//! [`set_mock`], or fail.

use serde_json::{Value, json};

/// Sends any request: `host.*`, or a Litus command id (`layer.zoomTo`, `select.byAttributes`…),
/// with its parameters. `litus-cli commands` lists the commands.
pub fn call(op: &str, params: Value) -> Result<Value, String> {
    crate::abi::host_call(op, &params)
}

/// Litus's version, the ABI, and the plugin's id and permissions.
pub fn info() -> Result<Value, String> {
    call("host.info", json!({}))
}

/// The map's layers: `[{id, name, raster, visible, geometry, features, selected, active,
/// fields}]`.
pub fn layers() -> Result<Value, String> {
    call("host.layers", json!({}))
}

/// The features of the layer called `layer` as GeoJSON; each feature has its `row`.
pub fn features(layer: &str) -> Result<Value, String> {
    call("host.features", json!({"layer": layer}))
}

/// The layer's selected features only, or at most `limit` of them.
pub fn features_with(layer: &str, selected: bool, limit: Option<usize>) -> Result<Value, String> {
    let mut p = json!({"layer": layer, "selected": selected});
    if let Some(n) = limit {
        p["limit"] = json!(n);
    }
    call("host.features", p)
}

/// The view: `{center: [lon, lat], scale, projection}`.
pub fn view() -> Result<Value, String> {
    call("host.view", json!({}))
}

/// Shows a message in the status bar.
pub fn notify(text: &str) {
    let _ = call("host.notify", json!({"text": text}));
}

/// Shows an error in the status bar.
pub fn notify_error(text: &str) {
    let _ = call("host.notify", json!({"text": text, "error": true}));
}

/// Adds a layer called `name` from a GeoJSON FeatureCollection (needs `edit`).
pub fn add_layer(name: &str, geojson: Value) -> Result<Value, String> {
    call("host.addLayer", json!({"name": name, "geojson": geojson}))
}

/// Logs a line (and, from a tool, adds it to the run's messages).
pub fn log(text: &str) {
    crate::abi::log(text);
}

/// A document read from the web by [`fetch`].
#[derive(Clone, Debug, PartialEq)]
pub struct Fetched {
    /// The address fetched.
    pub url: String,
    pub text: String,
    /// The document parsed, when it is a JSON object or array.
    pub json: Option<Value>,
    /// In the web app, an address not fetched yet: return straight away. Litus undoes the call
    /// and runs it again once the document has arrived.
    pub pending: bool,
}

/// Reads a small text document (at most 2 MB) with a GET request (needs `network`).
pub fn fetch(url: &str) -> Result<Fetched, String> {
    let v = call("host.fetch", json!({"url": url}))?;
    Ok(Fetched {
        url: v.get("url").and_then(Value::as_str).unwrap_or(url).to_string(),
        text: v.get("text").and_then(Value::as_str).unwrap_or_default().to_string(),
        json: v.get("json").cloned().filter(|j| !j.is_null()),
        pending: v.get("pending").and_then(Value::as_bool).unwrap_or(false),
    })
}

#[cfg(not(target_arch = "wasm32"))]
type Mock = Box<dyn FnMut(&str, &Value) -> Result<Value, String>>;

#[cfg(not(target_arch = "wasm32"))]
thread_local! {
    static MOCK: std::cell::RefCell<Option<Mock>> = const { std::cell::RefCell::new(None) };
}

/// For a plugin's own tests, off WebAssembly: answers this thread's host requests with `f`
/// (the op and its parameters).
#[cfg(not(target_arch = "wasm32"))]
pub fn set_mock(f: impl FnMut(&str, &Value) -> Result<Value, String> + 'static) {
    MOCK.with(|m| *m.borrow_mut() = Some(Box::new(f)));
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn mock_call(op: &str, params: &Value) -> Result<Value, String> {
    MOCK.with(|m| match m.borrow_mut().as_mut() {
        Some(f) => f(op, params),
        None => Err(format!("{op}: not running inside Litus (give a mock with litus_plugin::host::set_mock)")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_go_to_the_mock() {
        assert!(info().unwrap_err().contains("not running inside Litus"));
        let mut seen = Vec::new();
        set_mock(move |op, p| {
            seen.push(op.to_string());
            match op {
                "host.fetch" => Ok(json!({"url": p["url"], "text": "[1]", "json": [1]})),
                "host.layers" => Ok(json!([{"name": "A"}])),
                _ => Err(format!("no {op}")),
            }
        });
        assert_eq!(layers().unwrap()[0]["name"], "A");
        let f = fetch("https://example.com/a.json").unwrap();
        assert_eq!((f.text.as_str(), f.json, f.pending), ("[1]", Some(json!([1])), false));
        assert_eq!(view().unwrap_err(), "no host.view");
        notify("ignored errors");
    }
}
