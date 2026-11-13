# litus-plugin

The SDK for [Litus](https://litus.tellusai.com/) plugins: WebAssembly modules that add commands
and geoprocessing tools to the Litus desktop GIS.

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
litus-plugin = "0.1"
```

```rust
use litus_plugin::{Kind, Request, ToolOutput, Value, geojson, host, json};

fn handle(req: Request) -> Result<Value, String> {
    match (req.kind, req.id.as_str()) {
        (Kind::Command, "layer-report") => {
            let n = host::layers()?.as_array().map_or(0, Vec::len);
            host::notify(&format!("{n} layers"));
            Ok(json!({"layers": n}))
        }
        (Kind::Tool, "bounding-boxes") => {
            let boxes: Vec<Value> = req.features("input")?.iter().filter_map(|f| {
                let b = geojson::bbox(&geojson::positions(&f["geometry"]))?;
                Some(geojson::feature(geojson::polygon(&[[b[0], b[1]], [b[2], b[1]], [b[2], b[3]], [b[0], b[3]]]), json!({})))
            }).collect();
            Ok(ToolOutput::new().layer("Boxes", geojson::collection(boxes)).into())
        }
        (_, other) => Err(format!("unknown '{other}'")),
    }
}

litus_plugin::export!(handle, include_str!("../plugin.json"));
```

```sh
rustup target add wasm32-unknown-unknown   # once
cargo build --release --target wasm32-unknown-unknown
```

- **[The guide](docs/guide.md)** covers `plugin.json`, permissions, tool parameters, panels,
  tabs, translations, what a plugin can ask Litus for, packaging and registries.
- **[examples/hello-litus](examples/hello-litus)** is a complete plugin, with how to build,
  package and install it.
- Plugins in Python or any other language need no SDK: they run as
  [process plugins](docs/guide.md#process-plugins), speaking JSON lines over standard input and
  output.

## Testing a plugin

Off WebAssembly, in ordinary `cargo test` runs, `host::set_mock` answers the plugin's requests
to Litus, and `Request::from_value` builds a request to hand the handler directly.

## Compatibility

This SDK speaks version 1 of the Litus plugin ABI (`"abi": 1` in `plugin.json`). Litus keeps
ABI 1 working; changes to it only add to it. Set `minLitusVersion` in `plugin.json` to the
first Litus with the features a plugin uses.

## Licence

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT)
at your option. Plugins you build with it are yours, under the licence you choose.

Unless you explicitly state otherwise, any contribution you intentionally submit for inclusion
in this crate, as defined in the Apache-2.0 license, is dual-licensed as above, without any
additional terms or conditions.
