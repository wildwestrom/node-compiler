use std::{collections::HashMap, path::PathBuf};

use egui_snarl::{InPinId, OutPinId, Snarl, ui::PinInfo};
use serde::{Deserialize, Serialize};

// ─── Wire types ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireType {
    Bit,
    Byte,
    Word,
}

impl WireType {
    fn color(self) -> egui::Color32 {
        match self {
            WireType::Bit => egui::Color32::from_rgb(255, 200, 50), // yellow
            WireType::Byte => egui::Color32::from_rgb(100, 200, 255), // sky blue
            WireType::Word => egui::Color32::from_rgb(180, 100, 255), // purple
        }
    }

    pub(crate) fn pin_info(self) -> PinInfo {
        PinInfo::circle()
            .with_fill(self.color())
            .with_wire_color(self.color())
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            WireType::Bit => "Bit",
            WireType::Byte => "Byte",
            WireType::Word => "Word",
        }
    }
}

// ─── Runtime values ──────────────────────────────────────────────────────────
//
// WireType is the *type* on a wire connection; NodeValue is the *runtime value*.
// They mirror each other but serve different roles (type-level vs value-level).

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NodeValue {
    Bit(bool),
    Byte(u8),
    Word(u64),
    Bytes(Vec<u8>), // produced by Concat; maps to WireType::Byte
}

impl NodeValue {
    pub fn wire_type(&self) -> WireType {
        match self {
            NodeValue::Bit(_) => WireType::Bit,
            NodeValue::Byte(_) => WireType::Byte,
            NodeValue::Word(_) => WireType::Word,
            NodeValue::Bytes(_) => WireType::Byte,
        }
    }

    fn as_bit(&self) -> Option<bool> {
        if let NodeValue::Bit(v) = self {
            Some(*v)
        } else {
            None
        }
    }
    fn as_byte(&self) -> Option<u8> {
        if let NodeValue::Byte(v) = self {
            Some(*v)
        } else {
            None
        }
    }
    fn as_word(&self) -> Option<u64> {
        if let NodeValue::Word(v) = self {
            Some(*v)
        } else {
            None
        }
    }

    pub(crate) fn short_display(&self) -> String {
        match self {
            NodeValue::Bit(v) => {
                if *v {
                    "1".into()
                } else {
                    "0".into()
                }
            }
            NodeValue::Byte(v) => format!("{:#04X}", v),
            NodeValue::Word(v) => format!("{:#018X}", v),
            NodeValue::Bytes(v) => {
                let parts: Vec<String> = v.iter().take(8).map(|b| format!("{:02X}", b)).collect();
                let suffix = if v.len() > 8 {
                    format!(" …+{}", v.len() - 8)
                } else {
                    String::new()
                };
                format!("[{}{}]", parts.join(" "), suffix)
            }
        }
    }
}

// ─── Evaluator ───────────────────────────────────────────────────────────────

pub(crate) type EvalCache = HashMap<OutPinId, Option<NodeValue>>;

/// Evaluate every output pin in the graph. Returns a cache keyed by OutPinId.
pub(crate) fn eval_graph(snarl: &Snarl<NodeKind>, functions: &[FunctionDef]) -> EvalCache {
    let mut cache = EvalCache::new();
    for (node_id, node) in snarl.node_ids() {
        let n_out = node.output_count();
        for out in 0..n_out {
            eval_output(
                OutPinId {
                    node: node_id,
                    output: out,
                },
                snarl,
                &mut cache,
                functions,
            );
        }
    }
    cache
}

/// Memoised recursive evaluation of a single output pin.
fn eval_output(
    pin: OutPinId,
    snarl: &Snarl<NodeKind>,
    cache: &mut EvalCache,
    functions: &[FunctionDef],
) -> Option<NodeValue> {
    if let Some(cached) = cache.get(&pin) {
        return cached.clone();
    }
    cache.insert(pin, None); // cycle guard
    let result = compute(pin, snarl, cache, functions);
    *cache.get_mut(&pin).unwrap() = result.clone();
    result
}

/// Resolve the value arriving at input port `i` of `node`.
fn get_in(
    node: egui_snarl::NodeId,
    i: usize,
    snarl: &Snarl<NodeKind>,
    cache: &mut EvalCache,
    functions: &[FunctionDef],
) -> Option<NodeValue> {
    let in_pin = snarl.in_pin(InPinId { node, input: i });
    let &src = in_pin.remotes.first()?;
    eval_output(src, snarl, cache, functions)
}

/// Core per-node computation logic.
fn compute(
    pin: OutPinId,
    snarl: &Snarl<NodeKind>,
    cache: &mut EvalCache,
    functions: &[FunctionDef],
) -> Option<NodeValue> {
    let n = pin.node;
    let o = pin.output;

    // Helper closures — capture n/snarl/cache/functions.
    macro_rules! inp_bit {
        ($i:expr) => {
            get_in(n, $i, snarl, cache, functions)?.as_bit()?
        };
    }
    macro_rules! inp_byte {
        ($i:expr) => {
            get_in(n, $i, snarl, cache, functions)?.as_byte()?
        };
    }
    macro_rules! inp_word {
        ($i:expr) => {
            get_in(n, $i, snarl, cache, functions)?.as_word()?
        };
    }

    match &snarl[n] {
        // ── Values ────────────────────────────────────────────────────────
        NodeKind::Constant(values) => values.get(o).cloned(),

        // ── Bitwise — polymorphic on first input's type ───────────────────
        NodeKind::And => match get_in(n, 0, snarl, cache, functions)? {
            NodeValue::Bit(a) => Some(NodeValue::Bit(a & inp_bit!(1))),
            NodeValue::Byte(a) => Some(NodeValue::Byte(a & inp_byte!(1))),
            NodeValue::Word(a) => Some(NodeValue::Word(a & inp_word!(1))),
            _ => None,
        },
        NodeKind::Or => match get_in(n, 0, snarl, cache, functions)? {
            NodeValue::Bit(a) => Some(NodeValue::Bit(a | inp_bit!(1))),
            NodeValue::Byte(a) => Some(NodeValue::Byte(a | inp_byte!(1))),
            NodeValue::Word(a) => Some(NodeValue::Word(a | inp_word!(1))),
            _ => None,
        },
        NodeKind::Xor => match get_in(n, 0, snarl, cache, functions)? {
            NodeValue::Bit(a) => Some(NodeValue::Bit(a ^ inp_bit!(1))),
            NodeValue::Byte(a) => Some(NodeValue::Byte(a ^ inp_byte!(1))),
            NodeValue::Word(a) => Some(NodeValue::Word(a ^ inp_word!(1))),
            _ => None,
        },
        NodeKind::Not => match get_in(n, 0, snarl, cache, functions)? {
            NodeValue::Bit(a) => Some(NodeValue::Bit(!a)),
            NodeValue::Byte(a) => Some(NodeValue::Byte(!a)),
            NodeValue::Word(a) => Some(NodeValue::Word(!a)),
            _ => None,
        },
        NodeKind::Nand => match get_in(n, 0, snarl, cache, functions)? {
            NodeValue::Bit(a) => Some(NodeValue::Bit(!(a & inp_bit!(1)))),
            NodeValue::Byte(a) => Some(NodeValue::Byte(!(a & inp_byte!(1)))),
            NodeValue::Word(a) => Some(NodeValue::Word(!(a & inp_word!(1)))),
            _ => None,
        },
        NodeKind::Nor => match get_in(n, 0, snarl, cache, functions)? {
            NodeValue::Bit(a) => Some(NodeValue::Bit(!(a | inp_bit!(1)))),
            NodeValue::Byte(a) => Some(NodeValue::Byte(!(a | inp_byte!(1)))),
            NodeValue::Word(a) => Some(NodeValue::Word(!(a | inp_word!(1)))),
            _ => None,
        },

        // ── Shifts ────────────────────────────────────────────────────────
        NodeKind::Shl => {
            let amount = inp_byte!(1) as u32;
            match get_in(n, 0, snarl, cache, functions)? {
                NodeValue::Byte(a) => Some(NodeValue::Byte(a.wrapping_shl(amount))),
                NodeValue::Word(a) => Some(NodeValue::Word(a.wrapping_shl(amount))),
                _ => None,
            }
        }
        NodeKind::Shr => {
            let amount = inp_byte!(1) as u32;
            match get_in(n, 0, snarl, cache, functions)? {
                NodeValue::Byte(a) => Some(NodeValue::Byte(a.wrapping_shr(amount))),
                NodeValue::Word(a) => Some(NodeValue::Word(a.wrapping_shr(amount))),
                _ => None,
            }
        }

        // ── Arithmetic — polymorphic on Byte/Word ────────────────────────
        NodeKind::Add => match get_in(n, 0, snarl, cache, functions)? {
            NodeValue::Byte(a) => Some(NodeValue::Byte(a.wrapping_add(inp_byte!(1)))),
            NodeValue::Word(a) => Some(NodeValue::Word(a.wrapping_add(inp_word!(1)))),
            _ => None,
        },
        NodeKind::Sub => match get_in(n, 0, snarl, cache, functions)? {
            NodeValue::Byte(a) => Some(NodeValue::Byte(a.wrapping_sub(inp_byte!(1)))),
            NodeValue::Word(a) => Some(NodeValue::Word(a.wrapping_sub(inp_word!(1)))),
            _ => None,
        },
        NodeKind::Mul => match get_in(n, 0, snarl, cache, functions)? {
            NodeValue::Byte(a) => Some(NodeValue::Byte(a.wrapping_mul(inp_byte!(1)))),
            NodeValue::Word(a) => Some(NodeValue::Word(a.wrapping_mul(inp_word!(1)))),
            _ => None,
        },
        NodeKind::Div => match get_in(n, 0, snarl, cache, functions)? {
            NodeValue::Byte(a) => {
                let b = inp_byte!(1);
                if b == 0 {
                    None
                } else {
                    Some(NodeValue::Byte(a / b))
                }
            }
            NodeValue::Word(a) => {
                let b = inp_word!(1);
                if b == 0 {
                    None
                } else {
                    Some(NodeValue::Word(a / b))
                }
            }
            _ => None,
        },
        NodeKind::Mod => match get_in(n, 0, snarl, cache, functions)? {
            NodeValue::Byte(a) => {
                let b = inp_byte!(1);
                if b == 0 {
                    None
                } else {
                    Some(NodeValue::Byte(a % b))
                }
            }
            NodeValue::Word(a) => {
                let b = inp_word!(1);
                if b == 0 {
                    None
                } else {
                    Some(NodeValue::Word(a % b))
                }
            }
            _ => None,
        },

        // ── Byte manipulation ─────────────────────────────────────────────
        NodeKind::Concat { count } => {
            let count = *count as usize;
            let mut bytes = Vec::with_capacity(count);
            for i in 0..count {
                bytes.push(inp_byte!(i));
            }
            Some(NodeValue::Bytes(bytes))
        }
        NodeKind::Slice => {
            let word = inp_word!(0);
            let offset = inp_byte!(1);
            Some(NodeValue::Byte(
                ((word >> (offset as u32 * 8)) & 0xFF) as u8,
            ))
        }
        NodeKind::Pack => {
            let mut byte = 0u8;
            for i in 0..8u8 {
                if inp_bit!(i as usize) {
                    byte |= 1 << i;
                }
            }
            Some(NodeValue::Byte(byte))
        }
        NodeKind::Unpack => Some(NodeValue::Bit((inp_byte!(0) >> o) & 1 != 0)),

        // ── FunctionCall — evaluate the subgraph with injected inputs ─────
        NodeKind::FunctionCall { def_index, .. } => {
            let func = functions.get(*def_index)?;
            let sub = &func.graph;
            let mut sub_cache = EvalCache::new();

            // Inject parent-graph argument values at Source output pins.
            let mut arg_idx = 0;
            for (src_id, node) in sub.node_ids() {
                if matches!(node, NodeKind::Source { .. }) {
                    for out_port in 0..node.output_count() {
                        let val = get_in(n, arg_idx, snarl, cache, functions);
                        sub_cache.insert(
                            OutPinId {
                                node: src_id,
                                output: out_port,
                            },
                            val,
                        );
                        arg_idx += 1;
                    }
                }
            }

            // Evaluate the Sink's o-th input in the subgraph.
            for (sink_id, node) in sub.node_ids() {
                if matches!(node, NodeKind::Sink) {
                    let in_pin = sub.in_pin(InPinId {
                        node: sink_id,
                        input: o,
                    });
                    if let Some(&src) = in_pin.remotes.first() {
                        return eval_output(src, sub, &mut sub_cache, functions);
                    }
                }
            }
            None
        }

        _ => None, // Sink/Source have no computed outputs
    }
}

// ─── Function definitions ─────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FunctionDef {
    pub name: String,
    pub graph: Snarl<NodeKind>,
}

impl FunctionDef {
    pub fn new(name: impl Into<String>) -> Self {
        let mut graph = Snarl::new();
        // TODO: Source/Sink are placeholders for function arg/return terminals.
        // Dedicated FunctionArg/FunctionReturn variants are deferred.
        graph.insert_node(
            egui::pos2(-200.0, 0.0),
            NodeKind::Source {
                filename: "args".into(),
            },
        );
        graph.insert_node(egui::pos2(200.0, 0.0), NodeKind::Sink);
        FunctionDef {
            name: name.into(),
            graph,
        }
    }

    /// Display name: the given name, or a short structural hash when name is empty.
    pub fn display_name(&self) -> String {
        if self.name.is_empty() {
            format!("#{:08x}", self.graph_hash() as u32)
        } else {
            self.name.clone()
        }
    }

    /// Hash the subgraph structure: sorted node titles + input/output type counts.
    fn graph_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        let mut entries: Vec<String> = self
            .graph
            .node_ids()
            .map(|(_, node)| {
                format!(
                    "{}:{}:{}",
                    node.node_title(),
                    node.input_count(),
                    node.output_count()
                )
            })
            .collect();
        entries.sort_unstable();
        entries.hash(&mut hasher);
        hasher.finish()
    }

    /// Derive the FunctionCall port types from the subgraph's Source and Sink nodes.
    /// Source outputs → function inputs; Sink inputs → function outputs.
    pub fn call_types(&self) -> (Vec<WireType>, Vec<WireType>) {
        let mut in_types = vec![];
        let mut out_types = vec![];
        for (_, node) in self.graph.node_ids() {
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

// ─── Node kinds (mirrors spec discriminant catalogue) ────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NodeKind {
    Source {
        filename: PathBuf,
    },
    Sink,

    Constant(Vec<NodeValue>),

    And,
    Or,
    Xor,
    Not,
    Nand,
    Nor,
    Shl,
    Shr,

    Add,
    Sub,
    Mul,
    Div,
    Mod,

    Concat {
        count: u8,
    },
    Slice,
    Pack,
    Unpack,

    FunctionCall {
        def_index: usize,
        name: String, // cached for display; def_index is the authoritative link
        in_types: Vec<WireType>,
        out_types: Vec<WireType>,
    },
}

impl NodeKind {
    pub(crate) fn node_title(&self) -> String {
        match self {
            NodeKind::Source { .. } => "SOURCE".into(),
            NodeKind::Sink => "SINK".into(),
            NodeKind::Constant(_) => "CONSTANT".into(),
            NodeKind::And => "AND".into(),
            NodeKind::Or => "OR".into(),
            NodeKind::Xor => "XOR".into(),
            NodeKind::Not => "NOT".into(),
            NodeKind::Nand => "NAND".into(),
            NodeKind::Nor => "NOR".into(),
            NodeKind::Shl => "SHL".into(),
            NodeKind::Shr => "SHR".into(),
            NodeKind::Add => "ADD".into(),
            NodeKind::Sub => "SUB".into(),
            NodeKind::Mul => "MUL".into(),
            NodeKind::Div => "DIV".into(),
            NodeKind::Mod => "MOD".into(),
            NodeKind::Concat { count } => format!("CONCAT ({})", count),
            NodeKind::Slice => "SLICE".into(),
            NodeKind::Pack => "PACK".into(),
            NodeKind::Unpack => "UNPACK".into(),
            NodeKind::FunctionCall { name, .. } => name.clone(),
        }
    }

    pub(crate) fn input_count(&self) -> usize {
        match self {
            NodeKind::Source { .. } => 0,
            NodeKind::Sink => 1,
            NodeKind::Constant(_) => 0,
            NodeKind::Not | NodeKind::Unpack => 1,
            NodeKind::And
            | NodeKind::Or
            | NodeKind::Xor
            | NodeKind::Nand
            | NodeKind::Nor
            | NodeKind::Shl
            | NodeKind::Shr
            | NodeKind::Add
            | NodeKind::Sub
            | NodeKind::Mul
            | NodeKind::Div
            | NodeKind::Mod
            | NodeKind::Slice => 2,
            NodeKind::Pack => 8,
            NodeKind::Concat { count } => *count as usize,
            NodeKind::FunctionCall { in_types, .. } => in_types.len(),
        }
    }

    pub(crate) fn output_count(&self) -> usize {
        match self {
            NodeKind::Source { .. } => 1,
            NodeKind::Sink => 0,
            NodeKind::Constant(vals) => vals.len(),
            NodeKind::Unpack => 8,
            NodeKind::FunctionCall { out_types, .. } => out_types.len(),
            _ => 1,
        }
    }

    pub(crate) fn input_wire_type(&self, port: usize) -> WireType {
        match self {
            NodeKind::Sink | NodeKind::Concat { .. } => WireType::Byte,
            NodeKind::And
            | NodeKind::Or
            | NodeKind::Xor
            | NodeKind::Not
            | NodeKind::Nand
            | NodeKind::Nor
            | NodeKind::Shl
            | NodeKind::Shr
            | NodeKind::Add
            | NodeKind::Sub
            | NodeKind::Mul
            | NodeKind::Div
            | NodeKind::Mod => WireType::Byte,
            NodeKind::Slice => {
                if port == 0 {
                    WireType::Word
                } else {
                    WireType::Byte
                }
            }
            NodeKind::Pack => WireType::Bit,
            NodeKind::Unpack => WireType::Byte,
            NodeKind::FunctionCall { in_types, .. } => {
                in_types.get(port).copied().unwrap_or(WireType::Byte)
            }
            _ => WireType::Byte,
        }
    }

    pub(crate) fn output_wire_type(&self, port: usize) -> WireType {
        match self {
            NodeKind::Constant(values) => values
                .get(port)
                .map(|v| v.wire_type())
                .unwrap_or(WireType::Byte),
            NodeKind::Unpack => WireType::Bit,
            NodeKind::FunctionCall { out_types, .. } => {
                out_types.get(port).copied().unwrap_or(WireType::Byte)
            }
            _ => WireType::Byte,
        }
    }

    pub(crate) fn input_label(&self, port: usize) -> String {
        match self {
            NodeKind::Sink => "in".into(),
            NodeKind::And
            | NodeKind::Or
            | NodeKind::Xor
            | NodeKind::Nand
            | NodeKind::Nor
            | NodeKind::Add
            | NodeKind::Sub
            | NodeKind::Mul
            | NodeKind::Div
            | NodeKind::Mod => {
                if port == 0 {
                    "a".into()
                } else {
                    "b".into()
                }
            }
            NodeKind::Not => "a".into(),
            NodeKind::Shl | NodeKind::Shr => {
                if port == 0 {
                    "a".into()
                } else {
                    "amount".into()
                }
            }
            NodeKind::Concat { .. } => format!("in[{port}]"),
            NodeKind::Slice => {
                if port == 0 {
                    "in".into()
                } else {
                    "offset".into()
                }
            }
            NodeKind::Pack => format!("bit[{port}]"),
            NodeKind::Unpack => "in".into(),
            NodeKind::FunctionCall { .. } => format!("in[{port}]"),
            _ => format!("in[{port}]"),
        }
    }
}
