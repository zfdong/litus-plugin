# Hello Litus

An example Litus plugin built with the `litus-plugin` SDK: a command (**Layer Report**) that
counts the map's layers, and a tool (**Bounding Boxes**) that draws a box around each feature of
a layer, or one around them all. Its messages follow the interface language (German here).

## Build

```sh
rustup target add wasm32-unknown-unknown   # once
cargo build --release --target wasm32-unknown-unknown
```

The module is `target/wasm32-unknown-unknown/release/hello_litus.wasm`.

## Package

A package holds `plugin.json` and the module, named as the manifest's `entry` says
(`plugin.wasm`), zipped as `<id>-<version>.litusplugin`:

```sh
mkdir -p pkg
cp plugin.json pkg/
cp target/wasm32-unknown-unknown/release/hello_litus.wasm pkg/plugin.wasm
(cd pkg && zip ../hello-litus-0.1.0.litusplugin plugin.json plugin.wasm)
```

On Windows, `Compress-Archive -Path pkg\* -DestinationPath hello-litus-0.1.0.zip`, then rename
the `.zip` to `.litusplugin`.

Install it in **Plugins › Plugin Manager › Settings**, or drop the file on the Litus window.

## Test

The SDK's [`host::set_mock`](../../src/host.rs) answers host requests in ordinary
`cargo test` runs, and `Request::from_value` builds a request to hand the handler directly.
