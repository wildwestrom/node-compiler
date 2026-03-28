use crate::ui::logic::{FunctionDef, NodeKind};
use egui_snarl::Snarl;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub(crate) struct SavedState {
    pub(crate) snarl: Snarl<NodeKind>,
    pub(crate) functions: Vec<FunctionDef>,
}

pub(crate) fn save_state(state: &SavedState, path: &std::path::Path) -> anyhow::Result<()> {
    let bytes = postcard::to_allocvec(state)?;
    std::fs::write(path, bytes)?;
    Ok(())
}

pub(crate) fn load_state(path: &std::path::Path) -> anyhow::Result<SavedState> {
    let bytes = std::fs::read(path)?;
    Ok(postcard::from_bytes(&bytes)?)
}
