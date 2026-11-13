//! What a tool answers.

use serde_json::{Value, json};

/// A tool's result: new layers and standalone tables, or a selection, with messages for the
/// Geoprocessing history. Litus adds the results with undo, as for its own tools.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ToolOutput {
    layers: Vec<Value>,
    tables: Vec<Value>,
    selection: Option<Value>,
    messages: Vec<String>,
}

impl ToolOutput {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a layer called `name` from a GeoJSON FeatureCollection.
    pub fn layer(mut self, name: &str, geojson: Value) -> Self {
        self.layers.push(json!({"name": name, "geojson": geojson}));
        self
    }

    /// Adds a standalone table called `name`: a FeatureCollection whose features have no
    /// geometry (`null`).
    pub fn table(mut self, name: &str, geojson: Value) -> Self {
        self.tables.push(json!({"name": name, "geojson": geojson}));
        self
    }

    /// Selects `rows` (the features' `row`s) of the input layer `layer` instead of adding
    /// layers.
    pub fn selection(mut self, layer: &str, rows: impl IntoIterator<Item = usize>) -> Self {
        self.selection = Some(json!({"layer": layer, "rows": rows.into_iter().collect::<Vec<_>>()}));
        self
    }

    /// Adds a line to the Geoprocessing history.
    pub fn message(mut self, text: impl Into<String>) -> Self {
        self.messages.push(text.into());
        self
    }
}

impl From<ToolOutput> for Value {
    fn from(o: ToolOutput) -> Value {
        let mut v = json!({"layers": o.layers, "messages": o.messages});
        if !o.tables.is_empty() {
            v["tables"] = Value::from(o.tables);
        }
        if let Some(s) = o.selection {
            v["selection"] = s;
        }
        v
    }
}
