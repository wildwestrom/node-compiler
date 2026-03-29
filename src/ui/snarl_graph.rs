use std::collections::{HashMap, VecDeque};

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

/// Compute a DAG-layered layout for the nodes in `g`.
///
/// Returns `node_id → [x, y]` using the longest-path column assignment
/// (Kahn's topological sort) so data flows left → right.
fn layout_positions(g: &GraphData) -> HashMap<usize, [f32; 2]> {
    const COL_SPACING: f32 = 250.0;
    const ROW_SPACING: f32 = 120.0;

    // Build adjacency structures.
    let mut successors: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut in_degree: HashMap<usize, usize> = HashMap::new();

    for (id, _) in &g.nodes {
        in_degree.entry(*id).or_insert(0);
        successors.entry(*id).or_default();
    }
    for (out, inp) in &g.wires {
        successors.entry(out.node.0).or_default().push(inp.node.0);
        *in_degree.entry(inp.node.0).or_default() += 1;
    }

    // Longest-path column via Kahn's topological sort.
    // When a node is queued, all its predecessors have been processed,
    // so col[node] is already at its final maximum value.
    let mut col: HashMap<usize, usize> = g.nodes.iter().map(|(id, _)| (*id, 0)).collect();
    let mut remaining: HashMap<usize, usize> = in_degree.clone();

    let mut queue: VecDeque<usize> = remaining
        .iter()
        .filter(|&(_, &d)| d == 0)
        .map(|(id, _)| *id)
        .collect();

    while let Some(node_id) = queue.pop_front() {
        let node_col = col[&node_id];
        for &succ in successors.get(&node_id).into_iter().flatten() {
            let succ_col = col.entry(succ).or_insert(0);
            if node_col + 1 > *succ_col {
                *succ_col = node_col + 1;
            }
            let deg = remaining.entry(succ).or_insert(1);
            *deg -= 1;
            if *deg == 0 {
                queue.push_back(succ);
            }
        }
    }

    // Assign rows within each column in stable id order.
    let mut col_rows: HashMap<usize, usize> = HashMap::new();
    let mut positions: HashMap<usize, [f32; 2]> = HashMap::new();

    let mut by_col: Vec<(usize, usize)> =
        col.iter().map(|(&id, &c)| (id, c)).collect();
    by_col.sort_by_key(|&(id, c)| (c, id)); // deterministic within each column

    for (node_id, node_col) in by_col {
        let row = col_rows.entry(node_col).or_insert(0);
        positions.insert(node_id, [node_col as f32 * COL_SPACING, *row as f32 * ROW_SPACING]);
        *row += 1;
    }

    positions
}

/// Convert `GraphData` → `Snarl` with auto-arranged node positions.
///
/// Node IDs in `GraphData` may not be contiguous (deletions leave slab gaps).
/// We track `old_id → new SnarlNodeId` and rewire using the stored topology.
pub fn snarl_from_graph(g: &GraphData) -> Snarl<NodeKind> {
    let positions = layout_positions(g);
    let mut snarl = Snarl::new();
    let mut id_map: HashMap<usize, SnarlNodeId> = HashMap::new();

    for (old_id, kind) in &g.nodes {
        let [x, y] = positions.get(old_id).copied().unwrap_or([0.0, 0.0]);
        let new_id = snarl.insert_node(egui::pos2(x, y), kind.clone());
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

/// Re-apply the DAG-layered layout to an existing `Snarl` in-place.
pub fn auto_arrange(snarl: &mut Snarl<NodeKind>) {
    let g = graph_from_snarl(snarl);
    let positions = layout_positions(&g);
    for (old_id, _) in &g.nodes {
        if let Some(pos) = positions.get(old_id) {
            if let Some(info) = snarl.get_node_info_mut(SnarlNodeId(*old_id)) {
                info.pos = egui::pos2(pos[0], pos[1]);
            }
        }
    }
}

/// Convert `Snarl` → `GraphData`, preserving topology but discarding positions.
pub fn graph_from_snarl(snarl: &Snarl<NodeKind>) -> GraphData {
    let nodes = snarl.node_ids().map(|(id, n)| (id.0, n.clone())).collect();
    let wires = snarl.wires().map(|(out, inp)| (OutPinId::from(out), InPinId::from(inp))).collect();
    GraphData { nodes, wires }
}
