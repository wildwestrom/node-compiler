use crate::graph::{FunctionDef, GraphData};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub(crate) struct SavedState {
    pub(crate) root_graph: GraphData,
    pub(crate) functions: Vec<FunctionDef>,
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
