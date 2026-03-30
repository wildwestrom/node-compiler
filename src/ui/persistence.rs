use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::graph::{FunctionDef, GraphData};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub(crate) struct SavedState {
    pub(crate) root_graph: GraphData,
    pub(crate) functions: Vec<FunctionDef>,
}

/// Human-readable metadata stored alongside the main `.ncg` file.
///
/// Keyed by SHA3-512 hex hashes (functions) or NodeId (node names).
/// Missing on load → all names default to empty (graceful degradation).
#[derive(Clone, Serialize, Deserialize, Default)]
pub(crate) struct NamesData {
    /// function graph_hash → display name
    pub(crate) functions: BTreeMap<String, String>,
    /// root-graph NodeId → display name (for Constant nodes etc.)
    pub(crate) root_nodes: BTreeMap<usize, String>,
    /// subgraph node names: function_hash → (NodeId → name)
    pub(crate) subgraph_nodes: BTreeMap<String, BTreeMap<usize, String>>,
}

/// Returns the sidecar path for a given `.ncg` file: `foo.ncg` → `foo.ncg.names`.
pub(crate) fn sidecar_path(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(".names");
    PathBuf::from(s)
}

#[cfg(not(feature = "human-readable"))]
pub(crate) fn save_state(state: &SavedState, path: &std::path::Path) -> anyhow::Result<()> {
    let bytes = postcard::to_allocvec(state)?;
    std::fs::write(path, bytes)?;
    Ok(())
}

#[cfg(feature = "human-readable")]
pub(crate) fn save_state(state: &SavedState, path: &std::path::Path) -> anyhow::Result<()> {
    let text = ron::ser::to_string_pretty(state, Default::default())?;
    std::fs::write(path, text.as_bytes())?;
    Ok(())
}

#[cfg(not(feature = "human-readable"))]
pub(crate) fn load_state(path: &std::path::Path) -> anyhow::Result<SavedState> {
    let bytes = std::fs::read(path)?;
    Ok(postcard::from_bytes(&bytes)?)
}

#[cfg(feature = "human-readable")]
pub(crate) fn load_state(path: &std::path::Path) -> anyhow::Result<SavedState> {
    let text = std::fs::read_to_string(path)?;
    Ok(ron::from_str(&text)?)
}

#[cfg(not(feature = "human-readable"))]
pub(crate) fn save_names(names: &NamesData, path: &Path) -> anyhow::Result<()> {
    let bytes = postcard::to_allocvec(names)?;
    std::fs::write(path, bytes)?;
    Ok(())
}

#[cfg(feature = "human-readable")]
pub(crate) fn save_names(names: &NamesData, path: &Path) -> anyhow::Result<()> {
    let text = ron::ser::to_string_pretty(names, Default::default())?;
    std::fs::write(path, text.as_bytes())?;
    Ok(())
}

/// Load the sidecar names file. Returns `NamesData::default()` if the file is missing or invalid.
#[cfg(not(feature = "human-readable"))]
pub(crate) fn load_names(path: &Path) -> NamesData {
    std::fs::read(path)
        .ok()
        .and_then(|bytes| postcard::from_bytes(&bytes).ok())
        .unwrap_or_default()
}

/// Load the sidecar names file. Returns `NamesData::default()` if the file is missing or invalid.
#[cfg(feature = "human-readable")]
pub(crate) fn load_names(path: &Path) -> NamesData {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| ron::from_str(&text).ok())
        .unwrap_or_default()
}
