mod eval;
pub mod types;

pub use eval::{EvalCache, eval_graph};
pub use types::{NodeKind, NodeValue, WireType};

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

// ─── Graph ID types ──────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OutPinId {
    pub node: NodeId,
    pub output: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InPinId {
    pub node: NodeId,
    pub input: usize,
}

// ─── Graph trait ─────────────────────────────────────────────────────────────

/// Minimal read-only view over a node graph.
///
/// Implemented by both [`GraphData`] (pure data, no egui deps) and
/// `Snarl<NodeKind>` (live UI state, in `ui::snarl_graph`).
pub trait Graph<N> {
    fn nodes<'a>(&'a self) -> impl Iterator<Item = (NodeId, &'a N)>
    where
        N: 'a;
    fn node(&self, id: NodeId) -> &N;
    fn sources_of(&self, pin: InPinId) -> impl Iterator<Item = OutPinId> + '_;
}

// ─── GraphData — egui-free graph storage ─────────────────────────────────────

/// Serializable graph representation with no egui dependencies.
///
/// Node positions are not stored here; they live in `Snarl` when editing.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GraphData {
    /// `(stable_id, node_kind)` — IDs mirror `Snarl`'s slab keys.
    pub nodes: Vec<(usize, NodeKind)>,
    pub wires: Vec<(OutPinId, InPinId)>,
}

impl Graph<NodeKind> for GraphData {
    fn nodes<'a>(&'a self) -> impl Iterator<Item = (NodeId, &'a NodeKind)>
    where
        NodeKind: 'a,
    {
        self.nodes.iter().map(|(id, n)| (NodeId(*id), n))
    }

    fn node(&self, id: NodeId) -> &NodeKind {
        self.nodes
            .iter()
            .find(|(i, _)| *i == id.0)
            .map(|(_, n)| n)
            .expect("GraphData::node: unknown NodeId")
    }

    fn sources_of(&self, pin: InPinId) -> impl Iterator<Item = OutPinId> {
        self.wires
            .iter()
            .filter(move |(_, inp)| *inp == pin)
            .map(|(out, _)| *out)
    }
}

// ─── FunctionDef ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FunctionDef {
    pub graph: GraphData,
}

impl FunctionDef {
    pub fn new() -> Self {
        // Two nodes: Source (id=0) and Sink (id=1), not pre-connected.
        // Positions are set by the UI when opening the subgraph for editing.
        let graph = GraphData {
            nodes: vec![
                (
                    0,
                    NodeKind::Source {
                        filename: PathBuf::from("args"),
                    },
                ),
                (1, NodeKind::Sink),
            ],
            wires: vec![],
        };
        FunctionDef { graph }
    }

    /// Stable SHA3-512 content hash of the graph structure (hex string).
    pub fn graph_hash(&self) -> String {
        use sha3::{Digest, Sha3_512};
        let mut entries: Vec<String> = self
            .graph
            .nodes()
            .map(|(_, node): (NodeId, &NodeKind)| {
                format!(
                    "{}:{}:{}",
                    node.node_title(),
                    node.input_count(),
                    node.output_count()
                )
            })
            .collect();
        entries.sort_unstable();
        let combined = entries.join("\n");
        let hash = Sha3_512::digest(combined.as_bytes());
        format!("{hash:x}")
    }

    /// Derive the FunctionCall port types from the subgraph's Source and Sink nodes.
    pub fn call_types(&self) -> (Vec<WireType>, Vec<WireType>) {
        let mut in_types = vec![];
        let mut out_types = vec![];
        for (_, node) in self.graph.nodes() {
            match node {
                NodeKind::Source { .. } => {
                    for i in 0..node.output_count() {
                        in_types.push(node.output_wire_type(i));
                    }
                }
                NodeKind::Sink => {
                    for i in 0..node.input_count() {
                        out_types.push(node.input_wire_type(i));
                    }
                }
                _ => {}
            }
        }
        (in_types, out_types)
    }
}
