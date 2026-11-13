//! An example Litus plugin: a command and a tool. See `README.md` for building and packaging.

use litus_plugin::{Kind, Request, ToolOutput, Value, geojson, host, json};

fn handle(req: Request) -> Result<Value, String> {
    match (req.kind, req.id.as_str()) {
        (Kind::Command, "layer-report") => {
            let n = host::layers()?.as_array().map_or(0, Vec::len);
            host::notify(&req.tr("{n} layers").replace("{n}", &n.to_string()));
            Ok(json!({"layers": n}))
        }
        (Kind::Tool, "bounding-boxes") => {
            let features = req.features("input")?;
            let boxes: Vec<[f64; 4]> = if req.flag("whole") {
                let all: Vec<[f64; 2]> = features.iter().flat_map(|f| geojson::positions(&f["geometry"])).collect();
                geojson::bbox(&all).into_iter().collect()
            } else {
                features.iter().filter_map(|f| geojson::bbox(&geojson::positions(&f["geometry"]))).collect()
            };
            let out: Vec<Value> = boxes
                .iter()
                .map(|b| geojson::feature(geojson::polygon(&[[b[0], b[1]], [b[2], b[1]], [b[2], b[3]], [b[0], b[3]]]), json!({})))
                .collect();
            let n = out.len();
            Ok(ToolOutput::new().layer(&req.tr("Boxes"), geojson::collection(out)).message(format!("{n} boxes")).into())
        }
        (_, other) => Err(format!("unknown '{other}'")),
    }
}

litus_plugin::export!(handle, include_str!("../plugin.json"));
