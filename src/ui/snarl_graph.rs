use std::collections::HashMap;

use egui_snarl::{
    InPinId as SnarlIn, NodeId as SnarlNodeId, OutPinId as SnarlOut, Snarl,
};

use crate::graph::{Graph, GraphData, InPinId, NodeId, NodeKind, OutPinId};

// ─── ID conversions ──────────────────────────────────────────────────────────

impl From<SnarlNodeId> for NodeId {
    fn from(id: SnarlNodeId) -> Self {
        NodeId(id.0)
    }
}

impl From<NodeId> for SnarlNodeId {
    fn from(id: NodeId) -> Self {
        SnarlNodeId(id.0)
    }
}

impl From<SnarlOut> for OutPinId {
    fn from(id: SnarlOut) -> Self {
        OutPinId { node: NodeId(id.node.0), output: id.output }
    }
}

impl From<OutPinId> for SnarlOut {
    fn from(id: OutPinId) -> Self {
        SnarlOut { node: SnarlNodeId(id.node.0), output: id.output }
    }
}

impl From<SnarlIn> for InPinId {
    fn from(id: SnarlIn) -> Self {
        InPinId { node: NodeId(id.node.0), input: id.input }
    }
}

impl From<InPinId> for SnarlIn {
    fn from(id: InPinId) -> Self {
        SnarlIn { node: SnarlNodeId(id.node.0), input: id.input }
    }
}

// ─── Graph impl for Snarl ────────────────────────────────────────────────────

impl Graph<NodeKind> for Snarl<NodeKind> {
    fn nodes<'a>(&'a self) -> impl Iterator<Item = (NodeId, &'a NodeKind)> where NodeKind: 'a {
        self.node_ids().map(|(id, node)| (NodeId(id.0), node))
    }

    fn node(&self, id: NodeId) -> &NodeKind {
        &self[SnarlNodeId(id.0)]
    }

    fn sources_of(&self, pin: InPinId) -> impl Iterator<Item = OutPinId> {
        self.in_pin(SnarlIn::from(pin)).remotes.into_iter().map(OutPinId::from)
    }
}

// ─── Sync helpers ────────────────────────────────────────────────────────────

/// Convert `GraphData` → `Snarl`, placing all nodes at `(0,0)`.
///
/// Node IDs in the `GraphData` may not be contiguous (deletions leave gaps in
/// the slab). We track the mapping `old_id → new SnarlNodeId` and rewire.
pub fn snarl_from_graph(g: &GraphData) -> Snarl<NodeKind> {
    let mut snarl = Snarl::new();
    let mut id_map: HashMap<usize, SnarlNodeId> = HashMap::new();

    for (old_id, kind) in &g.nodes {
        let new_id = snarl.insert_node(egui::pos2(0.0, 0.0), kind.clone());
        id_map.insert(*old_id, new_id);
    }

    for (out, inp) in &g.wires {
        if let (Some(&new_out_node), Some(&new_inp_node)) =
            (id_map.get(&out.node.0), id_map.get(&inp.node.0))
        {
            snarl.connect(
                SnarlOut { node: new_out_node, output: out.output },
                SnarlIn { node: new_inp_node, input: inp.input },
            );
        }
    }

    snarl
}

/// Convert `Snarl` → `GraphData`, preserving topology but discarding positions.
pub fn graph_from_snarl(snarl: &Snarl<NodeKind>) -> GraphData {
    let nodes = snarl.node_ids().map(|(id, n)| (id.0, n.clone())).collect();
    let wires = snarl.wires().map(|(out, inp)| (OutPinId::from(out), InPinId::from(inp))).collect();
    GraphData { nodes, wires }
}
