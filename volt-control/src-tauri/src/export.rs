//! "导出设备信息": a read-only copy of the UASrv tree the app builds its model
//! from. Owners of other UA models send this file; it becomes a test fixture
//! (`tests/fixtures/*.json`, see the `fixture_*` tests in `model`) so support
//! for that model can be built and checked without the hardware.

use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use serde_json::{Map, Value, json};

use crate::model::children;
use crate::uasrv::{self, Client};

pub const FORMAT: &str = "volt-control-tree/1";

/// Subtrees discovery reads. `/` itself is stored without its other children
/// (logs, network, presets...), which the app never uses.
const ROOTS: [&str; 2] = ["/devices", "/auto_gain"];

/// Reads the tree over its own connection (only `get`, nothing is written).
pub async fn collect() -> Result<Value> {
    let (c, _pushes) = Client::connect(uasrv::DEFAULT_ADDR).await?;
    let mut nodes = Map::new();
    nodes.insert("/".into(), c.get("/").await?);
    for root in ROOTS {
        walk(&c, root, &mut nodes).await;
    }
    let device_type = nodes
        .iter()
        .find(|(p, _)| p.starts_with("/devices/") && p.matches('/').count() == 2)
        .and_then(|(_, d)| d["properties"]["device_type"]["value"].as_str())
        .ok_or_else(|| anyhow!(crate::i18n::t("err.no_ua_device").to_string()))?
        .to_string();
    Ok(json!({
        "format": FORMAT,
        "app_version": env!("CARGO_PKG_VERSION"),
        "device_type": device_type,
        "nodes": nodes,
    }))
}

/// Depth-first walk; a node that fails is recorded with its error, not skipped.
async fn walk(c: &Client, root: &str, nodes: &mut Map<String, Value>) {
    let mut stack = vec![root.to_string()];
    while let Some(path) = stack.pop() {
        match c.get(&path).await {
            Ok(data) => {
                for child in children(&data).into_iter().rev() {
                    stack.push(format!("{}/{child}", path.trim_end_matches('/')));
                }
                nodes.insert(path, data);
            }
            Err(e) => {
                nodes.insert(path, json!({ "error": e.to_string() }));
            }
        }
    }
}

/// Writes the export into `dir` as `VoltControl-<device type>.json`.
pub fn save(tree: &Value, dir: &Path) -> Result<PathBuf> {
    let kind = tree["device_type"].as_str().unwrap_or("unknown");
    let path = dir.join(format!("VoltControl-{kind}.json"));
    std::fs::write(&path, serde_json::to_vec_pretty(tree)?)?;
    Ok(path)
}
