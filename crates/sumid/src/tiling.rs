//! MultiFrame via KWin custom tiling: a one-shot KWin script builds the chosen
//! tile layout on an output, then reports every output's tile leaves back to
//! sumid (`TileLayoutReport`), which is how layouts are verified and read.

use std::sync::mpsc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sumi_core::multiframe;
use zbus::blocking::Connection;

use crate::manager::Manager;

const KWIN: &str = "org.kde.KWin";
const PLUGIN: &str = "sumi-tiles";

/// `__OUTPUT__` is a JSON string (or null = read only), `__TREE__` a tile tree
/// (or null = clear the output's tiles).
const TEMPLATE: &str = r#"// Sumi MultiFrame: build a tile layout, then report all outputs' tiles.
var TARGET = __OUTPUT__;
var TREE = __TREE__;
function clear(root) {
    for (var guard = 0; root.tiles.length > 0 && guard < 64; guard++) root.tiles[root.tiles.length - 1].remove();
}
function build(tile, node) {
    if (!node.children || node.children.length < 2) return;
    tile.split(node.dir);
    for (var guard = 0; tile.tiles.length < node.children.length && guard < 16; guard++)
        tile.tiles[tile.tiles.length - 1].split(node.dir);
    var kids = tile.tiles;
    for (var i = 0; i < node.children.length && i < kids.length; i++) {
        var r = node.children[i].rect, g = kids[i].relativeGeometry;
        g.x = r[0]; g.y = r[1]; g.width = r[2]; g.height = r[3];
        kids[i].relativeGeometry = g;
    }
    for (var i = 0; i < node.children.length && i < kids.length; i++) build(kids[i], node.children[i]);
}
function leaves(tile, out) {
    if (tile.tiles.length == 0) { var g = tile.relativeGeometry; out.push([g.x, g.y, g.width, g.height]); }
    for (var i = 0; i < tile.tiles.length; i++) leaves(tile.tiles[i], out);
    return out;
}
var report = [];
var error = "";
try {
    for (var s = 0; s < workspace.screens.length; s++) {
        var out = workspace.screens[s];
        var root = workspace.tilingForScreen(out).rootTile;
        if (TARGET !== null && out.name == TARGET) {
            clear(root);
            if (TREE !== null) build(root, TREE);
        }
        report.push({ output: String(out.name), leaves: root.tiles.length ? leaves(root, []) : [] });
    }
} catch (e) { error = String(e); }
callDBus("io.github.JenAriDev.Sumi", "/io/github/JenAriDev/Sumi", "io.github.JenAriDev.Sumi1",
         "TileLayoutReport", JSON.stringify({ outputs: report, error: error }));
"#;

#[derive(Debug, Clone, Deserialize)]
struct Report {
    outputs: Vec<OutputTiles>,
    error: String,
}

#[derive(Debug, Clone, Deserialize)]
struct OutputTiles {
    output: String,
    leaves: Vec<[f64; 4]>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutputLayout {
    pub output: String,
    /// Width / height of the output as KScreen lays it out (rotation applied).
    pub aspect: f64,
    /// DisplayWidget layout number currently in KWin's tiles, 0 = no tiles,
    /// null = some other (custom) tile layout.
    pub layout: Option<u8>,
    pub tiles: usize,
}

/// Aspect ratios of the enabled outputs, from `kscreen-doctor -j`.
fn aspects() -> Result<Vec<(String, f64)>> {
    let out = std::process::Command::new("kscreen-doctor").arg("-j").output().context("running kscreen-doctor")?;
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).context("parsing kscreen-doctor -j")?;
    Ok(v["outputs"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|o| o["enabled"].as_bool() == Some(true))
        .filter_map(|o| {
            let w = o["size"]["width"].as_f64()?;
            let h = o["size"]["height"].as_f64()?;
            Some((o["name"].as_str()?.to_string(), if h > 0.0 { w / h } else { 1.0 }))
        })
        .collect())
}

impl Manager {
    /// Called back by the tiling script.
    pub fn on_tile_report(&self, json: String) {
        if let Some(tx) = self.tile_reply.lock().unwrap().take() {
            let _ = tx.send(json);
        }
    }

    /// Run the tiling script and wait for its report.
    fn run_tiles_script(&self, target: Option<&str>, tree: Option<&multiframe::TileNode>) -> Result<Report> {
        let _serial = self.tile_lock.lock().unwrap();
        let conn = Connection::session()?;
        let script = TEMPLATE
            .replace("__OUTPUT__", &serde_json::to_string(&target)?)
            .replace("__TREE__", &serde_json::to_string(&tree)?);
        let path = dirs::runtime_dir().or_else(dirs::cache_dir).unwrap_or_else(std::env::temp_dir).join("sumi-tiles.js");
        std::fs::write(&path, script)?;
        let (tx, rx) = mpsc::channel();
        *self.tile_reply.lock().unwrap() = Some(tx);
        let _ = conn.call_method(Some(KWIN), "/Scripting", Some("org.kde.kwin.Scripting"), "unloadScript", &(PLUGIN,));
        let id: i32 = conn
            .call_method(Some(KWIN), "/Scripting", Some("org.kde.kwin.Scripting"), "loadScript", &(path.to_string_lossy().as_ref(), PLUGIN))
            .context("KWin scripting (is this a Plasma 6 session?)")?
            .body()
            .deserialize()?;
        let obj = format!("/Scripting/Script{id}");
        if conn.call_method(Some(KWIN), obj.as_str(), Some("org.kde.kwin.Script"), "run", &()).is_err() {
            conn.call_method(Some(KWIN), "/Scripting", Some("org.kde.kwin.Scripting"), "start", &())?;
        }
        let reply = rx.recv_timeout(Duration::from_secs(3));
        let _ = conn.call_method(Some(KWIN), "/Scripting", Some("org.kde.kwin.Scripting"), "unloadScript", &(PLUGIN,));
        self.tile_reply.lock().unwrap().take();
        let json = reply.context("KWin did not run the tiling script")?;
        let r: Report = serde_json::from_str(&json).context("parsing the tiling report")?;
        if !r.error.is_empty() {
            bail!("KWin tiling script: {}", r.error);
        }
        Ok(r)
    }

    /// Every enabled output's current tile layout.
    pub fn tile_layouts(&self) -> Result<Vec<OutputLayout>> {
        let aspects = aspects()?;
        let report = self.run_tiles_script(None, None)?;
        Ok(describe(&report, &aspects))
    }

    /// Put DisplayWidget layout `n` (1–13) on an output; 0 removes its tiles.
    pub fn set_tile_layout(&self, output: &str, n: u8) -> Result<Vec<OutputLayout>> {
        if self.read_only {
            bail!("sumid is running read-only");
        }
        let aspects = aspects()?;
        let aspect = aspects.iter().find(|(o, _)| o == output).map(|(_, a)| *a).with_context(|| format!("no enabled output {output:?}"))?;
        let tree = if n == 0 { None } else { Some(multiframe::layout(n, aspect).with_context(|| format!("no layout {n}"))?) };
        let report = self.run_tiles_script(Some(output), tree.as_ref())?;
        let layouts = describe(&report, &aspects);
        let got = layouts.iter().find(|l| l.output == output).and_then(|l| l.layout);
        if got != Some(n) {
            bail!("{output}: asked KWin for layout {n}, it reports {got:?}");
        }
        tracing::info!("{output}: MultiFrame layout {n}");
        Ok(layouts)
    }
}

fn describe(report: &Report, aspects: &[(String, f64)]) -> Vec<OutputLayout> {
    report
        .outputs
        .iter()
        .map(|o| {
            let aspect = aspects.iter().find(|(n, _)| *n == o.output).map(|(_, a)| *a).unwrap_or(16.0 / 9.0);
            let layout = if o.leaves.is_empty() { Some(0) } else { multiframe::identify(&o.leaves, aspect) };
            OutputLayout { output: o.output.clone(), aspect, layout, tiles: o.leaves.len() }
        })
        .collect()
}
