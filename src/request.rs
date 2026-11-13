//! A request from Litus: a command, a tool, or a parameter's list of choices.

use base64::Engine;
use serde_json::{Map, Value};

/// What Litus asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A command: from the ribbon, command search, a panel, a script or `litus-cli`.
    Command,
    /// A geoprocessing tool. It gets its layers and files in the request and makes no host
    /// requests.
    Tool,
    /// The list of choices for a tool's `choice` parameter with `choicesFrom` ([`Request::param`]
    /// names it). Answer a list of texts.
    Choices,
}

/// A file a tool's `file` parameter was given.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct File {
    /// Its name, without a folder (`run.nc`).
    pub name: String,
    pub data: Vec<u8>,
}

/// A request from Litus.
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    pub kind: Kind,
    /// The command's or tool's id, as in `plugin.json`.
    pub id: String,
    /// The parameters, by name.
    pub params: Value,
    /// A tool's layers: each layer parameter's layer name to its GeoJSON FeatureCollection.
    pub layers: Map<String, Value>,
    /// A tool's `file` parameters, by parameter name: `{"name", "data"}`, the data in base64.
    /// [`Request::file`] decodes one.
    pub files: Map<String, Value>,
    /// The interface language's code (`en`, `de`, `zh-CN`…).
    pub lang: String,
    /// For [`Kind::Choices`]: the parameter whose list is wanted.
    pub param: Option<String>,
    /// For a plugin with a panel: `{"values": {...}, "changed": [...]}`, else `Null`.
    pub panel: Value,
    /// The request as Litus sent it.
    pub raw: Value,
}

impl Request {
    /// Reads a request, as Litus sends it. Useful in a plugin's own tests, to call the handler
    /// directly.
    pub fn from_value(raw: Value) -> Result<Request, String> {
        let kind = match raw.get("kind").and_then(Value::as_str) {
            Some("command") => Kind::Command,
            Some("tool") => Kind::Tool,
            Some("choices") => Kind::Choices,
            other => return Err(format!("unknown request kind {other:?}: this plugin may need a newer litus-plugin SDK")),
        };
        if let Some(abi) = raw.get("abi").and_then(Value::as_u64)
            && abi != u64::from(crate::ABI)
        {
            return Err(format!("Litus speaks plugin ABI {abi}; this plugin was built for ABI {}", crate::ABI));
        }
        let text = |k: &str| raw.get(k).and_then(Value::as_str).map(str::to_string);
        let object = |k: &str| raw.get(k).and_then(Value::as_object).cloned().unwrap_or_default();
        Ok(Request {
            kind,
            id: text("id").unwrap_or_default(),
            params: raw.get("params").cloned().filter(Value::is_object).unwrap_or_else(|| Value::Object(Map::new())),
            layers: object("layers"),
            files: object("files"),
            lang: text("lang").unwrap_or_else(|| "en".into()),
            param: text("param"),
            panel: raw.get("panel").cloned().unwrap_or(Value::Null),
            raw,
        })
    }

    /// The parameter `name`, if it was given (and isn't `null`).
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.params.get(name).filter(|v| !v.is_null())
    }

    /// The text parameter `name`.
    pub fn text(&self, name: &str) -> Option<&str> {
        self.get(name).and_then(Value::as_str)
    }

    /// The number parameter `name` (a number, or text holding one).
    pub fn number(&self, name: &str) -> Option<f64> {
        let v = self.get(name)?;
        v.as_f64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
    }

    /// The `bool` parameter `name`, `false` when it wasn't given.
    pub fn flag(&self, name: &str) -> bool {
        self.get(name).and_then(Value::as_bool).unwrap_or(false)
    }

    /// The `extent` parameter `name`: a longitude/latitude box `[xmin, ymin, xmax, ymax]`.
    pub fn extent(&self, name: &str) -> Option<[f64; 4]> {
        let a = self.get(name)?.as_array()?;
        match a.iter().map(Value::as_f64).collect::<Option<Vec<f64>>>()?.as_slice() {
            &[x0, y0, x1, y1] => Some([x0, y0, x1, y1]),
            _ => None,
        }
    }

    /// The FeatureCollection of the layer parameter `name` (only its selected features when the
    /// layer has a selection).
    pub fn layer(&self, name: &str) -> Result<&Value, String> {
        let layer = self.text(name).filter(|n| !n.is_empty()).ok_or_else(|| format!("no layer was given for '{name}'"))?;
        self.layers.get(layer).ok_or_else(|| format!("Litus sent no features for the layer '{layer}'"))
    }

    /// The features of the layer parameter `name`.
    pub fn features(&self, name: &str) -> Result<&[Value], String> {
        Ok(self.layer(name)?.get("features").and_then(Value::as_array).map_or(&[], Vec::as_slice))
    }

    /// The file the `file` parameter `name` was given, decoded.
    pub fn file(&self, name: &str) -> Result<File, String> {
        let f = self.files.get(name).ok_or_else(|| format!("no file was given for '{name}'"))?;
        let data = f.get("data").and_then(Value::as_str).unwrap_or_default();
        let data =
            base64::engine::general_purpose::STANDARD.decode(data).map_err(|e| format!("{name}: the file's data isn't base64: {e}"))?;
        Ok(File { name: f.get("name").and_then(Value::as_str).unwrap_or_default().to_string(), data })
    }

    /// A panel checkbox's value.
    pub fn panel_bool(&self, id: &str) -> Option<bool> {
        self.panel.get("values")?.get(id)?.as_bool()
    }

    /// A panel slider's value.
    pub fn panel_int(&self, id: &str) -> Option<i64> {
        self.panel.get("values")?.get(id)?.as_i64()
    }

    /// Whether this call set the panel widget `id` (the one the user just moved).
    pub fn panel_changed(&self, id: &str) -> bool {
        self.panel.get("changed").and_then(Value::as_array).is_some_and(|a| a.iter().any(|v| v.as_str() == Some(id)))
    }

    /// `s` in the interface language, from the manifest's `i18n` table (given to
    /// [`export!`](crate::export)), or `s` itself. A code with a region matches its language:
    /// `zh-CN` finds `zh`, and `zh` finds `zh-CN`.
    pub fn tr(&self, s: &str) -> String {
        crate::translate::tr(&self.lang, s)
    }
}
