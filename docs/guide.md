# Writing Litus plugins

Plugins add commands and geoprocessing tools to [Litus](https://litus.tellusai.com/). A plugin is
a WebAssembly module with a `plugin.json` manifest, packaged as a `.litusplugin` file, or a
program on the user's computer (a [process plugin](#process-plugins)), or a set of skills for
the AI Assistant.

Users install a `.litusplugin` file in **Plugins › Plugin Manager › Settings** (or drop it on
the Litus window), load a plugin from its folder while developing it, or install from a plugin
registry: Litus's own at `https://litus.tellusai.com/api/plugins/registry.json`, or one an
organisation hosts for its own plugins (see [Plugin registries](#plugin-registries)).

> **Note.** Litus ships today as the desktop app and `litus-cli` with its `commands` and `run`
> subcommands. What this guide says of a web app, and of other `litus-cli` subcommands
> (`plugins`, `run --plugin`, `run --sample`), describes planned parts of Litus.

## Safety

Plugins run in a sandbox: an interpreter (`wasmi`) with no access to files, the network, the
clock or anything else outside the module. A plugin reaches Litus only through the requests
described below, and Litus checks each one:

- Every plugin can **read the map**: its layers, their features and the view.
- With the **`edit`** permission, it can change the project. It can run commands that edit it and
  add layers. A plugin command that changes the project is one undo step, and it is rolled back
  if the command fails.
- With the **`files`** permission, it can run commands that read or write files the user names,
  and its tools can take a file the user picks (a [`file` parameter](#file-parameters)).
- With the **`network`** permission, it can reach the web: read a small document from a web
  address ([`host.fetch`](#fetching-a-document)) and run the commands that fetch (adding data
  or services by their address). With `edit` too, it can add web tile layers (`map.addTiles`).
  The addresses are the plugin's choice and can carry what it read from the project, so this is
  a permission of its own. A command that would bring in a tile layer some other way (importing
  a map file) has it taken out again without `network`.
- No plugin may run the user's scripts, tasks or models, install or locate other software,
  manage plugins, replace the project, or step through undo history.

A [process plugin](#process-plugins) is the exception: it is a program on your computer, not
sandboxed, and the install prompt says so.

The install prompt lists these permissions. Each call has a fuel budget, so a plugin stuck in a
loop is stopped. Memory is capped at 512 MB, and requests and replies are size-limited. A plugin
that traps, panics or returns garbage produces an error message, never a crash.

The registry is curated: plugins are reviewed before they are listed, and packages are
hash-pinned. A `blocklist.json` beside the registry withdraws a plugin (every version, or one
package) from every install. Litus checks it when you browse the registry and at startup.

## Writing a plugin

A plugin is a `cdylib` crate that uses the `litus-plugin` SDK:

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
litus-plugin = "0.1"

[profile.release]
opt-level = "s"   # a smaller module
lto = true
strip = true
```

The handler gets each request and answers it:

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

litus_plugin::export!(handle);
```

```sh
rustup target add wasm32-unknown-unknown   # once
cargo build --release --target wasm32-unknown-unknown
```

What the SDK gives:

- `Request`: the request's `kind` (`Command`, `Tool` or `Choices`), `id`, `params`, `lang`
  and, for tools, `layers` and `files`. Helpers read parameters (`text`, `number`, `flag`,
  `extent`), a layer parameter's `features`, a `file` parameter's decoded `file`, and a panel's
  values (`panel_bool`, `panel_int`, `panel_changed`). `req.tr` translates a message with the
  manifest's `i18n` table when the manifest is given to the macro:
  `export!(handle, include_str!("../plugin.json"))`.
- `ToolOutput`: a tool's answer, built from `layer`, `table`, `selection` and `message`.
- `host`: the [requests a command can make](#what-a-plugin-can-ask-for), as `info`, `layers`,
  `features`, `view`, `notify`, `add_layer`, `fetch` and `log`, and any other with `call`.
- `geojson`: making features and geometries, and reading coordinates (`positions`, `bbox`).
- For a plugin's own tests, off WebAssembly: `Request::from_value` builds a request to hand the
  handler, and `host::set_mock` answers host requests.

A plugin's panic stops its module: Litus reports the error and logs the panic's message.
[`examples/hello-litus`](../examples/hello-litus) is a complete plugin, with how to build and package
it.

### `plugin.json`

```json
{
  "id": "hello-litus",
  "name": "Hello Litus",
  "version": "0.1.0",
  "description": "What it does, in a sentence or two.",
  "author": "You",
  "homepage": "https://github.com/you/your-plugin",
  "license": "MIT",
  "entry": "plugin.wasm",
  "abi": 1,
  "minLitusVersion": "0.1.0",
  "permissions": ["edit"],
  "commands": [
    { "id": "layer-report", "label": "Layer\nReport", "help": "Shown on hover.", "icon": "stats", "params": "{}" }
  ],
  "tools": [
    {
      "id": "bounding-boxes", "name": "Bounding Boxes", "summary": "One line.", "toolbox": "Hello",
      "params": [
        { "name": "input", "label": "Input features", "kind": "vectorLayer", "required": true },
        { "name": "whole", "label": "One box for the whole layer", "kind": "bool", "default": false }
      ]
    }
  ],
  "i18n": { "de": { "Layer\nReport": "Ebenen-\nbericht" } }
}
```

- `id`: lower-case letters, digits and hyphens. Commands and tools get ids
  `plugin:<id>/<command>`, which the ribbon, command search, scripts and `litus-cli` use.
- `version` and `minLitusVersion` look like `1.2.3`. Litus refuses a plugin that needs a
  newer Litus.
- `permissions`: `edit`, `files` and `network` (see [Safety](#safety)). Ask only for what you use.
- `commands[].icon`: the name of one of Litus's own icons (`stats`, `zoom_full`, `bookmark`,
  `buffer`…), or a puzzle piece when it isn't one.
- Tool parameter `kind`s: `layer`, `vectorLayer`, `polygonLayer`, `pointLayer`, `field`,
  `numericField`, `number`, `text`, `choice` (with `choices`), `bool`, `file` and `folder` (see
  [File parameters](#file-parameters)).
- `panel` (optional) gives the plugin a pane of its own, with widgets bound to its commands. See
  [A panel](#a-panel).
- `i18n` translates the plugin's own text into the interface language, by language code (`de`,
  `zh-CN`…). See [Translations](#translations).
- `skills` (optional) lists skills for the AI Assistant that the package carries. See
  [Skills](#skills).

A package is a zip archive named `<id>-<version>.litusplugin`. It holds `plugin.json` and the module
at its root, or inside one top-level folder. Packages are at most 64 MB, and modules at most 48 MB.

### File parameters

A tool can read a file the user picks, such as a NetCDF model run, with a `file` parameter.
The plugin needs the `files` permission:

```json
{ "name": "model", "label": "Model output", "kind": "file", "required": true, "extensions": ["nc", "nc4"] }
```

- The Tools pane shows a path box with a **Browse** button. `extensions` (without the dot; up
  to 16) filters the file picker; leave it out for any file.
- On the desktop and the command line the value is a path, and Litus reads the file. In the
  web app, which has no paths, the user picks the file and its contents go with the run. A
  script can do the same with `{"name": "run.nc", "data": base64}`.
- The tool gets the file in the request's `files`, by parameter name, as
  `{"name": "run.nc", "data": base64}`. The parameter itself holds just the file name.
- Files are at most 64 MB. The Geoprocessing history keeps the file's name, not its contents.
- Litus versions before file parameters refuse a plugin that has one, so set
  `minLitusVersion` to the first version with them.

A `folder` parameter names a folder for a process plugin to read or write, such as where to
save downloads. It also needs the `files` permission:

```json
{ "name": "out_folder", "label": "Output folder", "kind": "folder", "required": true }
```

- On the desktop the Tools pane shows a path box with a **Browse** button that opens a folder
  picker, starting in the folder already given. The web app, which has no paths, shows just the
  box.
- The tool gets the path as text in `params`. Litus doesn't read the folder or check it
  exists: the plugin does, and makes it when it should.

### Extent parameters

An `extent` parameter is an area to work in, as ArcGIS Pro's Extent parameter is:

```json
{ "name": "extent", "label": "Extent", "kind": "extent", "required": true }
```

- The Tools pane offers **Current Map Extent** (what the map shows; the default unless the
  parameter has another) and the map's vector layers.
- The tool gets a longitude/latitude box `[xmin, ymin, xmax, ymax]` in `params`, never the
  name: the map's extent when the request is made, or a layer's extent (its selected features,
  if it has a selection). A script or the assistant can also give a box itself.
- A choice that takes its list from an extent (`choicesFrom`, below) follows the map: the Tools
  pane asks again once the map has stopped moving for a moment.
- On a globe, the map's extent is the whole world.

### Choices that depend on other inputs

A `choice` parameter can take its list from the plugin as the user fills in the form, such as
the datasets that cover the polygons picked above it. `choicesFrom` names the parameters the
list depends on:

```json
{ "name": "in_polygon", "label": "Area", "kind": "polygonLayer", "required": true },
{ "name": "dataset", "label": "Dataset", "kind": "choice", "choicesFrom": ["in_polygon"], "required": true }
```

- Once every parameter in `choicesFrom` has a value, and again whenever one changes, the Tools
  pane asks the plugin for the list. It shows "Getting the choices…" meanwhile, and the
  plugin's error with a **Try Again** button when it fails. Until then the list holds the
  `choices` declared, which may be left out.
- The request is a tool's, with `"kind": "choices"` and `param`, the parameter whose list is
  wanted: `{"abi", "kind": "choices", "id": "<tool>", "param": "dataset", "params": {...},
  "layers": {...}, "lang"}`. `params` holds the form's values so far, and `layers` the layers
  among them as GeoJSON (no files). Like a tool, it makes no host requests.
- The answer is a list of texts, `{"ok": ["2022 CoNED Topobathy", "2019 NGS Topobathy"]}`, or
  `{"ok": {"choices": [...]}}`. At most 2,000 are kept, each up to 500 characters; repeats and
  empty texts are dropped. It gets as long as a command does (2 minutes for a process plugin).
- A value the new list doesn't have is cleared. Litus doesn't check a run's value against the
  list (a script or the assistant may give one), so the tool checks it, and should say what the
  choices are when it doesn't know it.
- In the desktop app the plugin is asked on a thread of its own, so the window stays usable.
- Litus versions before `choicesFrom` refuse a choice parameter with no `choices`.

### Translations

A plugin speaks the interface language through its `i18n` table: for each language code, its
English strings and their translations, written exactly as they appear in `plugin.json`
(`\n` included):

```json
"i18n": {
  "de": { "Bounding Boxes": "Begrenzungsrahmen", "Input features": "Eingabe-Features" },
  "zh-CN": { "Bounding Boxes": "边界框", "Input features": "输入要素" }
}
```

- It covers the plugin's name and description, command labels and help, tool names, summaries
  and toolboxes, parameter labels and `choice` values, the tab, group and menu labels, and the
  panel's title, help and widget labels. A string with no translation shows in English.
- Litus's interface languages are `de`, `es`, `fr`, `it`, `nl`, `pt`, `ru`, `tr`, `az`, `id`,
  `vi`, `zh`, `ja` and `ko`. A code with a region matches its language: `zh-CN` serves `zh`.
- The registry copies the name and description translations into `registry.json`, so the
  Plugin Manager lists plugins in the interface language before they are installed.
- Messages a plugin builds while it runs can follow the interface language too: every request
  carries its code as `lang`. In the SDK that is `req.lang`, and `req.tr` looks a message up in
  `i18n`.
- Up to 64 languages and 20,000 translated strings in all.

### A tab of its own

By default a plugin's commands and tools get a group on the **Plugins** tab. A plugin can have a
ribbon tab of its own instead. The tab sits at the end of the tab strip, and **Settings › Customize › Ribbon**
can hide it like any other. Declare it in `plugin.json`:

```json
"tab": {
  "label": "Explorer",
  "groups": [
    {
      "label": "Go To",
      "items": [
        { "menu": "Continent", "icon": "zoom_world", "help": "Go to a continent.", "items": ["go-africa", "go-europe"] },
        "random-place"
      ]
    },
    { "label": "Inspect", "items": ["layer-report", "tool:field-summary"] }
  ]
}
```

- `items` lists the buttons. An item is a command id, `tool:<id>` (which opens that tool in the
  Tools pane), `panel` (which opens the plugin's [panel](#a-panel)), or a dropdown `menu` whose
  `items` are any of those.
- A tab has 1 to 8 groups, a group has 1 to 16 items, and a menu has 1 to 32 entries. Every
  entry must name one of the plugin's own commands or tools.
- The label can't be the name of one of Litus's tabs (Map, Insert, Analysis, Plugins…). The
  `i18n` translations apply to the tab, group and menu labels too.


### A panel

A plugin can have a panel: a pane on the right of the map with texts, checkboxes, sliders and
buttons bound to its commands. Declare it in `plugin.json`:

```json
"panel": {
  "title": "Historical Imagery",
  "icon": "history",
  "help": "Step through the archived releases of World Imagery.",
  "state": "panel-state",
  "items": [
    { "kind": "text", "id": "selected", "label": "Selected release" },
    { "kind": "checkbox", "id": "local", "label": "Only versions with local changes", "command": "panel-state" },
    { "kind": "slider", "id": "release", "label": "Release timeline", "steps": true, "command": "go-to-release" },
    { "kind": "button", "command": "older" },
    { "kind": "button", "command": "newer" },
    { "kind": "text", "id": "metadata", "label": "Location metadata" }
  ]
}
```

- `title` is the pane's tab and the label of the button that opens it, which Litus puts
  first in the plugin's group on the **Plugins** tab. On a tab of its own, put `panel` among a
  group's items. Command search lists the panel too. `icon` and `help` are for that button.
- `items` lists the widgets, top to bottom (1 to 32). Buttons that follow each other share a
  row. Each has an `id` (lower-case letters, digits and hyphens; a button's is its command's
  unless given), and `label` and `help` (on hover) where they apply:
    - `text`: a caption (`label`) over text the plugin sets (`text` is what it shows first).
    - `checkbox`: a box with a `label`, `default` `false` unless given.
    - `slider`: whole numbers from `min` to `max` (`default` is `min` unless given), with a
      caption, the value's label, and its two ends' labels. `steps: true` adds ‹ and › buttons
      that move it by one.
    - `button`: runs its `command`; its `label` is the command's unless given.
- A checkbox runs its `command` when it changes, and a slider when it is let go, or stepped:
  never while it is dragged, while the pane shows the label of the value it would take. Without
  a `command`, a widget's value just goes with the plugin's next request.
- `state` (optional) is a command Litus runs each time the panel is opened, to answer its
  state.
- Every command and widget must be one of the plugin's own commands. A plugin with a panel
  can't have a command called `panel`.

A plugin keeps nothing between calls, so Litus keeps the panel's state. Every request of a
plugin with a panel carries the widgets' values, whatever ran the command (the panel, a ribbon
button, a script or the command line):

```json
"panel": { "values": { "local": false, "release": 196 }, "changed": ["release"] }
```

`values` has each checkbox (`true` or `false`) and slider (a whole number), and `changed` the
ones this call set: the widget the user just moved. A widget's command is run with
`{"panel": {"release": 196}}` as its parameters, which sets the values before the call and is
taken out of the request's `params`, so a script or `litus-cli` can do the same:

```sh
litus-cli run --sample --cmd 'plugin:historical-imagery/go-to-release={"panel": {"release": 40}}'
```

Any command can answer the panel's new state, beside the rest of its answer:

```json
{
  "panel": {
    "values": { "release": 196 },
    "items": {
      "selected": { "text": "2025-12-18\nRelease 13192" },
      "release": { "min": 0, "max": 196, "labels": ["2014-02-20", "…", "2025-12-18"] },
      "newer": { "enabled": false },
      "metadata": { "text": "Source: Maxar (Vivid)\nResolution: 0.3 m" }
    }
  }
}
```

- An item can set `label` (`null` brings back the declared one), `enabled` and `visible`; a text
  its `text`; a slider its `min`, `max` and `labels`, one per value from `min` (the numbers when
  there are none). What an answer leaves out stays as it was.
- Litus checks the answer like any other: unknown ids and fields and values of the wrong type
  are ignored, a slider's value stays within its ends, texts are clipped to 4,000 characters,
  labels to 200, and a slider has at most 2,000 labels of 80 characters.
- A plugin words what it sets in the request's `lang`; Litus translates only what the
  manifest declares.
- A command that fails leaves the values as they were. The panel's state command is no undo
  step when it changes nothing in the project (it only reads the map); other commands keep
  theirs.
- Litus asks for the state only when the panel opens, and takes in what the plugin's own
  commands answer. It doesn't run the plugin when the map changes in other ways: undo, another
  plugin, a project opened. A state command that reads what it shows from the map (which
  release is on it) puts the panel right when it is opened again.
- The values are kept while Litus runs, not saved with the project.
- `plugin.panel {"id": "<plugin>"}` answers a panel's state (`values` and `items`), and sets
  `values` or, with `"refresh": true`, runs the state command first. On the command line, which
  has no panes, the commands still run with the values given.
- Litus versions before panels ignore `panel` (and refuse a tab that names it): keep the
  commands usable from the ribbon too, or set `minLitusVersion`.

### What a plugin can ask for

Inside a request, `host::call(op, params)` sends one of these to Litus:

| Request | Answer |
|---|---|
| `host.info` | Litus's version, the ABI, the plugin's id and permissions |
| `host.layers` | `[{id, name, raster, visible, geometry, features, selected, active, fields: [{name, type}]}]` |
| `host.features` `{layer, selected?, limit?}` | the layer's features as GeoJSON; each feature has its `row` |
| `host.view` | `{center: [lon, lat], scale, projection}` |
| `host.notify` `{text, error?}` | shows a message in the status bar |
| `host.addLayer` `{name, geojson}` | adds a layer (`edit`) |
| `host.fetch` `{url}` | `{url, text, json?}`: the document at a web address (`network`; see [Fetching a document](#fetching-a-document)) |
| `map.addTiles` `{url, kind?, name?, attribution?, maxZoom?, overlay?, replace?, blend?}` | adds a web tile layer (`edit` and `network`) |
| any command id (`layer.zoomTo`, `select.byAttributes`, `layer.symbology`…) | what the command returns, within the plugin's permissions |

In the web app a command can't wait for a download, so where `layer.addUrl` has to download a
file whole it answers `{"pending": true, "url": …}` there: Litus downloads the data off the
frame and adds the layer when it lands, as **Home › Add Data › From URL** does. A PMTiles
archive, a cloud-optimised GeoTIFF or a FlatGeobuf or GeoParquet read in place is added at once,
as elsewhere.

`litus-cli commands` lists every command id and its parameters.

#### Fetching a document

`host.fetch {"url": "https://…"}` reads a small text document, such as a web API's JSON
answer, with a GET request. It needs the `network` permission. The answer is
`{"url", "text", "json"?}`: `url` is the address fetched, `text` the document, and `json` the
document parsed when it is a JSON object or array.

- Only `http://` and `https://` addresses on the internet: not this computer, the local network
  or cloud metadata addresses, and no redirect may lead there or from `https` to `http`. Names
  are checked as written and again where they resolve, on every redirect too, so a name that
  resolves to this computer or the local network is refused on the desktop and the command line
  (in the web app, the browser decides what it may reach). Requests carry no credentials (an
  S3 connection's included) and no headers of the plugin's. On the desktop and the command line
  they never go through a proxy (`HTTPS_PROXY` and the like are ignored), since a proxy would
  reach addresses Litus can't check.
- At most 2 MB, in UTF-8 (a byte-order mark is dropped). An answer that isn't a success (404,
  500…) is an error ending in `HTTP <status>`.
- Answers are kept for 10 minutes (up to 256 documents and 16 MB in all) while Litus runs,
  so asking for the same address again costs nothing. Each plugin has answers of its own: one
  never reads what another fetched. Errors are kept for 30 seconds,
  so every command waiting for an address sees its error; after that it is fetched again.
- On the desktop and the command line the request waits for the answer: at most 20 seconds for
  one, and 60 seconds for all the fetches of one plugin call. A call makes at most 64 fetches.
- In the web app nothing may wait. A fetch that isn't kept yet answers
  `{"pending": true, "url"}` and starts in the background. The command should then return
  straight away, before doing anything outside the project (writing a file): Litus undoes
  what the call did to the project (its notices stay, and so does anything done outside it),
  and runs the command
  again with the same parameters once every pending answer has landed (up to 8 per call), when
  `host.fetch` answers from what was kept. A command that walks a chain of documents, each
  address taken from the one before, runs once per step, up to 64 times. The browser follows
  redirects itself, and the server must allow requests from web pages (CORS).

In the SDK this is `host::fetch(url)`, whose answer has `pending` set in that case.

Tools work differently. A tool receives its layer parameters' features as GeoJSON (only the
selected features when a layer has a selection), and its `file` parameters' files. It returns new layers, standalone tables or a
selection, plus messages for the Geoprocessing history. Litus adds the results as it does for
built-in tools, with undo. Tools can log, but they make no host requests.

In the desktop app a plugin tool runs in the background, so the map and the rest of the window
stay usable: the Tools pane shows the line the tool last logged, or a process plugin's last
`progress` message, and a **Cancel** button. Cancel stops a process plugin's program; a WebAssembly tool runs on to its
end and its result is dropped. One plugin tool runs at a time. `gp.run`, scripts and
`litus-cli` still wait for the result.

### The ABI (version 1)

The SDK implements this; it is described here for other languages.

The module is `wasm32` with no imports except these three, in module `litus`:

- `call(ptr: i32, len: i32) -> i32` runs the host request at `ptr`
  (`{"op": "...", "params": {...}}`). It returns the length of the reply, which is
  `{"ok": value}` or `{"error": "text"}`, or `-1` when there is none.
- `result(ptr: i32)` copies that reply to `ptr`. Allocate the length `call` returned first.
- `log(ptr: i32, len: i32)` logs a UTF-8 line.

The module exports:

- `memory`.
- `litus_alloc(len: i32) -> i32`, which returns room for `len` bytes. Litus writes the
  request there.
- `litus_call(ptr: i32, len: i32) -> i64`, which handles the request and returns
  `reply_ptr << 32 | reply_len`. The reply is `{"ok": value}` or `{"error": "text"}`.

Requests are `{"abi": 1, "kind": "command" | "tool" | "choices", "id": "...", "params": {...}, "layers": {...}, "lang": "de"}`
(`choices` asks for a parameter's list: see [Choices that depend on other inputs](#choices-that-depend-on-other-inputs)).
`lang` is the interface language's code (`en` for English). Commands of a plugin with a
[panel](#a-panel) also get `panel`, and may answer `panel` beside the rest of their `ok` value.
Tools also get `layers`, which maps each layer's name to its FeatureCollection, and, when
they have `file` parameters, `files`, which maps each one's name to `{"name", "data"}` with the
file's contents in base64. A tool's `ok`
value is `{"layers": [{"name", "geojson"}], "tables": [...], "selection": {"layer", "rows"},
"messages": [...]}`.

Each call gets a fresh instance, so a module keeps no state between calls and need not free
memory. The fuel budget is about 2×10⁹ instructions for a command and 4×10¹⁰ for a tool.

## Process plugins

A process plugin is a program on your computer instead of a WebAssembly module: a Python script,
or an executable in any language. It can use the file system, threads and any library, which
lets existing analysis code (numpy, xarray, GDAL…) become a plugin without a rewrite. It is **not
sandboxed**: it can do anything you can. The install prompt says so, and the Plugin Manager
marks it *Not sandboxed*, in **Browse** too, before it is installed (with the Python version
its environment downloads, if it has one). Process plugins run on the desktop and the command
line, not in the web app: its **Browse** list shows them as *Desktop and command line only*,
with no **Install** button.

```json
{
  "id": "hello-python",
  "name": "Hello Python",
  "version": "0.1.0",
  "runtime": "process",
  "run": { "windows": ["python", "main.py"], "default": ["python3", "main.py"] },
  "permissions": [],
  "commands": [{ "id": "layer-count", "label": "Count\nLayers" }],
  "tools": [{
    "id": "feature-count", "name": "Count Features", "summary": "Counts a layer's features.",
    "params": [{ "name": "input", "label": "Input layer", "kind": "vectorLayer", "required": true }]
  }]
}
```

- `"runtime": "process"` and `run`: the program and its arguments, started in the plugin's
  folder. A bare name (`python3`) is looked up on `PATH`; a relative path (`bin/tool`) is in the
  plugin's folder. `run` can name a program per platform (`windows`, `macos`, `linux`, and
  `default` for the rest). There is no `entry`.
- Commands, tools, tabs, translations and permissions work as for WebAssembly plugins.
  Permissions still decide which host requests Litus answers, but they don't limit what the
  program itself can do.
- A `.litusplugin` holds the plugin's files (the manifest, scripts, executables). Installing it
  unpacks them to `unpacked/<id>/` in the plugins folder, keeping executables executable. A
  plugin folder installed for development (Plugin Manager › Settings, or
  `litus-cli plugins install ./folder`) runs where it is.

### The protocol

Each command or tool call starts the program once. It speaks the [ABI](#the-abi-version-1)'s
requests and replies over its standard input and output, one JSON object per line:

1. Litus writes the request: `{"abi", "kind", "id", "params", "layers", "files", "lang"}`, as
   a WebAssembly module gets it.
2. The program writes lines:
    - `{"call": {"op": "...", "params": {...}}}`: a [host request](#what-a-plugin-can-ask-for).
      Litus answers with one line, `{"ok": value}` or `{"error": "text"}`.
    - `{"log": "text"}`: a line for the log (and a tool's run messages).
    - `{"progress": "text"}`: what a tool is doing now, shown while it runs and not kept in its
      messages: a line per step of a long run, say.
    - Last, `{"ok": value}` or `{"error": "text"}`: the result.
3. Litus then closes the program's input; a program still running a few seconds later is
   stopped.

Output lines that aren't protocol messages are logged and skipped, since libraries print (even
JSON), and so is the program's error output. A command is stopped after 2 minutes; a tool runs
until it finishes or is cancelled (the desktop app runs it in the background: see
[What a plugin can ask for](#what-a-plugin-can-ask-for)).

The plugin above, in Python with no SDK:

```python
import json
import sys


def call(op, params):
    """Sends a request to Litus and returns its answer."""
    print(json.dumps({"call": {"op": op, "params": params}}), flush=True)
    reply = json.loads(sys.stdin.readline())
    if "error" in reply:
        raise RuntimeError(reply["error"])
    return reply["ok"]


req = json.loads(sys.stdin.readline())
try:
    if req["id"] == "layer-count":
        n = len(call("host.layers", {}))
        call("host.notify", {"text": f"The map has {n} layers."})
        result = {"ok": {"layers": n}}
    elif req["id"] == "feature-count":
        layer = req["params"]["input"]
        n = len(req["layers"][layer]["features"])
        result = {"ok": {"layers": [], "messages": [f"{layer} has {n} features."]}}
    else:
        result = {"error": f"unknown command '{req['id']}'"}
except Exception as e:
    result = {"error": str(e)}
print(json.dumps(result), flush=True)
```

### Python environments

A Python plugin can have Litus make its environment, so its users don't install Python or
any package themselves:

```json
"runtime": "process",
"python": { "version": "3.12", "requirements": "requirements.txt" },
"run": ["python", "main.py"]
```

- `python` in `run` is then that environment's interpreter, on every platform.
- Litus makes the environment when the plugin is installed: in the background in the desktop
  app (the Plugin Manager shows how it's going, and a **Prepare** button makes it again), and
  before the command returns in `litus-cli plugins install` and `get`.
  `litus-cli plugins prepare <id>` and the `plugin.prepare` command make it on demand.
- It uses [uv](https://github.com/astral-sh/uv), which Litus downloads once (a pinned version,
  checked against its pinned SHA-256; `LITUS_UV` names one to use instead, which is then
  trusted as it is, unchecked). uv installs a standalone Python of the version asked for and
  the packages in the requirements file, from PyPI. Pythons and uv's cache are shared in
  `tools/` in the plugins folder; each plugin's environment is in `envs/<id>/`. One
  environment is made at a time per plugin, even across the app and `litus-cli`.
- The environment is made again when the Python version or the requirements change (a plugin
  update), and removed with the plugin. Until it's ready, the plugin's calls fail with a
  message saying how to prepare it.
- The first install downloads Python and the packages: tens to hundreds of MB, depending on
  them. The install prompt says so.

## Skills

A plugin can carry [skills](user-guide/assistant.md#skills) for the AI Assistant: instructions
for a kind of task, which the assistant reads when a request needs them. Put each in
`skills/<name>/SKILL.md` in the package and list the names in `plugin.json`:

```json
"skills": ["flood-map"]
```

A `SKILL.md` starts with front matter giving the same `name` and a `description` that says when
to use it, then the instructions in Markdown (the
[Agent Skills](https://agentskills.io) format). The assistant knows it as `<plugin id>:<name>`
(`hello-litus:flood-map`) while the plugin is enabled. A `SKILL.md` is at most 64 KB, and a
plugin has at most 32 skills.

A package of skills alone, with no code, says `"runtime": "skills"`. It has no module, commands,
tools, tab or permissions. Nothing in it runs, and it installs and works like any other plugin,
in the desktop app, on the command line and in the web app:

```json
{
  "id": "flood-skills",
  "name": "Flood mapping skills",
  "version": "1.0.0",
  "runtime": "skills",
  "skills": ["flood-map", "gauge-report"]
}
```

## Plugin registries

A registry is a `registry.json` file at an HTTPS address, with the packages it lists.

Each plugin's entry in the published `registry.json` comes from its manifest: `id`, `name`,
`version`, `description`, `author`, `homepage`, `license`, `categories`, `permissions`,
`minLitusVersion` and `i18n` (its name and description translated), with `package` (an HTTPS
address, or a path relative to `registry.json`), `sha256` and `size`. A process plugin's entry
also has `"runtime": "process"` and, when it has a Python environment, `"python": "3.12"` (the
version); a WebAssembly plugin's has neither, and a skills package's has `"runtime": "skills"`.
Litus installs a package only if it runs the way its entry says. `litus-cli plugins browse`
prints each entry with `runtime` filled in (`wasm` when the registry leaves it out).

## Sharing a plugin

- **Yourself or your team:** give people the `.litusplugin` file, or host a registry of your own
  (above) and have them set its address under **Plugin Manager › Settings**.
- **Everyone:** plugins in Litus's registry are reviewed before they are listed. To submit one,
  contact Tellus Consulting through [litus.tellusai.com](https://litus.tellusai.com/).
