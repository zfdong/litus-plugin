//! Requests read as Litus sends them, and tool answers as Litus reads them.

use litus_plugin::{Kind, Request, ToolOutput, Value, json};

#[test]
fn a_tool_request() {
    let req = Request::from_value(json!({
        "abi": 1, "kind": "tool", "id": "boxes", "lang": "de",
        "params": {"input": "Countries", "size": "2.5", "whole": true, "extent": [-10, 40, 5, 52], "model": "run.nc"},
        "layers": {"Countries": {"type": "FeatureCollection", "features": [{"type": "Feature", "row": 3, "geometry": null, "properties": {}}]}},
        "files": {"model": {"name": "run.nc", "data": "aGk="}}
    }))
    .unwrap();
    assert_eq!((req.kind, req.id.as_str(), req.lang.as_str()), (Kind::Tool, "boxes", "de"));
    assert_eq!(req.features("input").unwrap().len(), 1);
    assert_eq!(req.number("size"), Some(2.5));
    assert!(req.flag("whole") && !req.flag("missing"));
    assert_eq!(req.extent("extent"), Some([-10.0, 40.0, 5.0, 52.0]));
    let f = req.file("model").unwrap();
    assert_eq!((f.name.as_str(), f.data.as_slice()), ("run.nc", b"hi".as_slice()));
    assert!(req.features("nope").unwrap_err().contains("no layer was given"));
    // No manifest was given to export!, so messages stay in English.
    assert_eq!(req.tr("Done"), "Done");
}

#[test]
fn commands_choices_and_panels() {
    let req = Request::from_value(json!({"abi": 1, "kind": "command", "id": "go", "params": null,
        "panel": {"values": {"local": true, "release": 196}, "changed": ["release"]}}))
    .unwrap();
    assert_eq!((req.kind, req.params.clone(), req.lang.as_str()), (Kind::Command, json!({}), "en"));
    assert_eq!((req.panel_bool("local"), req.panel_int("release")), (Some(true), Some(196)));
    assert!(req.panel_changed("release") && !req.panel_changed("local"));
    let c = Request::from_value(json!({"abi": 1, "kind": "choices", "id": "t", "param": "dataset"})).unwrap();
    assert_eq!((c.kind, c.param.as_deref()), (Kind::Choices, Some("dataset")));
    assert!(Request::from_value(json!({"abi": 1, "kind": "later"})).unwrap_err().contains("newer litus-plugin SDK"));
    assert!(Request::from_value(json!({"abi": 2, "kind": "command"})).unwrap_err().contains("ABI 2"));
}

#[test]
fn tool_output() {
    let fc = json!({"type": "FeatureCollection", "features": []});
    let v: Value = ToolOutput::new().layer("Boxes", fc.clone()).table("Stats", fc.clone()).message("2 boxes").into();
    assert_eq!(
        v,
        json!({"layers": [{"name": "Boxes", "geojson": fc}], "tables": [{"name": "Stats", "geojson": fc}], "messages": ["2 boxes"]})
    );
    let v: Value = ToolOutput::new().selection("Countries", [1, 4]).into();
    assert_eq!(v["selection"], json!({"layer": "Countries", "rows": [1, 4]}));
}
