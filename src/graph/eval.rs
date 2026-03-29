use std::collections::HashMap;

use crate::graph::{FunctionDef, Graph, InPinId, NodeId, OutPinId};
use crate::graph::types::{NodeKind, NodeValue};

pub type EvalCache = HashMap<OutPinId, Option<NodeValue>>;

/// Evaluate every output pin in the graph. Returns a cache keyed by [`OutPinId`].
pub fn eval_graph<G: Graph<NodeKind>>(graph: &G, functions: &[FunctionDef]) -> EvalCache {
    let mut cache = EvalCache::new();
    for (node_id, node) in graph.nodes() {
        for out in 0..node.output_count() {
            eval_output(OutPinId { node: node_id, output: out }, graph, &mut cache, functions);
        }
    }
    cache
}

/// Memoised recursive evaluation of a single output pin.
fn eval_output<G: Graph<NodeKind>>(
    pin: OutPinId,
    graph: &G,
    cache: &mut EvalCache,
    functions: &[FunctionDef],
) -> Option<NodeValue> {
    if let Some(cached) = cache.get(&pin) {
        return cached.clone();
    }
    cache.insert(pin, None); // cycle guard
    let result = compute(pin, graph, cache, functions);
    *cache.get_mut(&pin).unwrap() = result.clone();
    result
}

/// Resolve the value arriving at input port `i` of `node`.
fn get_in<G: Graph<NodeKind>>(
    node: NodeId,
    i: usize,
    graph: &G,
    cache: &mut EvalCache,
    functions: &[FunctionDef],
) -> Option<NodeValue> {
    let src = graph.sources_of(InPinId { node, input: i }).next()?;
    eval_output(src, graph, cache, functions)
}

/// Core per-node computation logic.
fn compute<G: Graph<NodeKind>>(
    pin: OutPinId,
    graph: &G,
    cache: &mut EvalCache,
    functions: &[FunctionDef],
) -> Option<NodeValue> {
    let n = pin.node;
    let o = pin.output;

    macro_rules! inp_bit {
        ($i:expr) => {
            get_in(n, $i, graph, cache, functions)?.as_bit()?
        };
    }
    macro_rules! inp_byte {
        ($i:expr) => {
            get_in(n, $i, graph, cache, functions)?.as_byte()?
        };
    }
    macro_rules! inp_word {
        ($i:expr) => {
            get_in(n, $i, graph, cache, functions)?.as_word()?
        };
    }

    match graph.node(n) {
        // ── Values ────────────────────────────────────────────────────────
        NodeKind::Constant(values) => values.get(o).cloned(),

        // ── Bitwise — polymorphic on first input's type ───────────────────
        NodeKind::And => match get_in(n, 0, graph, cache, functions)? {
            NodeValue::Bit(a) => Some(NodeValue::Bit(a & inp_bit!(1))),
            NodeValue::Byte(a) => Some(NodeValue::Byte(a & inp_byte!(1))),
            NodeValue::Word(a) => Some(NodeValue::Word(a & inp_word!(1))),
            _ => None,
        },
        NodeKind::Or => match get_in(n, 0, graph, cache, functions)? {
            NodeValue::Bit(a) => Some(NodeValue::Bit(a | inp_bit!(1))),
            NodeValue::Byte(a) => Some(NodeValue::Byte(a | inp_byte!(1))),
            NodeValue::Word(a) => Some(NodeValue::Word(a | inp_word!(1))),
            _ => None,
        },
        NodeKind::Xor => match get_in(n, 0, graph, cache, functions)? {
            NodeValue::Bit(a) => Some(NodeValue::Bit(a ^ inp_bit!(1))),
            NodeValue::Byte(a) => Some(NodeValue::Byte(a ^ inp_byte!(1))),
            NodeValue::Word(a) => Some(NodeValue::Word(a ^ inp_word!(1))),
            _ => None,
        },
        NodeKind::Not => match get_in(n, 0, graph, cache, functions)? {
            NodeValue::Bit(a) => Some(NodeValue::Bit(!a)),
            NodeValue::Byte(a) => Some(NodeValue::Byte(!a)),
            NodeValue::Word(a) => Some(NodeValue::Word(!a)),
            _ => None,
        },
        NodeKind::Nand => match get_in(n, 0, graph, cache, functions)? {
            NodeValue::Bit(a) => Some(NodeValue::Bit(!(a & inp_bit!(1)))),
            NodeValue::Byte(a) => Some(NodeValue::Byte(!(a & inp_byte!(1)))),
            NodeValue::Word(a) => Some(NodeValue::Word(!(a & inp_word!(1)))),
            _ => None,
        },
        NodeKind::Nor => match get_in(n, 0, graph, cache, functions)? {
            NodeValue::Bit(a) => Some(NodeValue::Bit(!(a | inp_bit!(1)))),
            NodeValue::Byte(a) => Some(NodeValue::Byte(!(a | inp_byte!(1)))),
            NodeValue::Word(a) => Some(NodeValue::Word(!(a | inp_word!(1)))),
            _ => None,
        },

        // ── Shifts ────────────────────────────────────────────────────────
        NodeKind::Shl => {
            let amount = inp_byte!(1) as u32;
            match get_in(n, 0, graph, cache, functions)? {
                NodeValue::Byte(a) => Some(NodeValue::Byte(a.wrapping_shl(amount))),
                NodeValue::Word(a) => Some(NodeValue::Word(a.wrapping_shl(amount))),
                _ => None,
            }
        }
        NodeKind::Shr => {
            let amount = inp_byte!(1) as u32;
            match get_in(n, 0, graph, cache, functions)? {
                NodeValue::Byte(a) => Some(NodeValue::Byte(a.wrapping_shr(amount))),
                NodeValue::Word(a) => Some(NodeValue::Word(a.wrapping_shr(amount))),
                _ => None,
            }
        }

        // ── Arithmetic — polymorphic on Byte/Word ────────────────────────
        NodeKind::Add => match get_in(n, 0, graph, cache, functions)? {
            NodeValue::Byte(a) => Some(NodeValue::Byte(a.wrapping_add(inp_byte!(1)))),
            NodeValue::Word(a) => Some(NodeValue::Word(a.wrapping_add(inp_word!(1)))),
            _ => None,
        },
        NodeKind::Sub => match get_in(n, 0, graph, cache, functions)? {
            NodeValue::Byte(a) => Some(NodeValue::Byte(a.wrapping_sub(inp_byte!(1)))),
            NodeValue::Word(a) => Some(NodeValue::Word(a.wrapping_sub(inp_word!(1)))),
            _ => None,
        },
        NodeKind::Mul => match get_in(n, 0, graph, cache, functions)? {
            NodeValue::Byte(a) => Some(NodeValue::Byte(a.wrapping_mul(inp_byte!(1)))),
            NodeValue::Word(a) => Some(NodeValue::Word(a.wrapping_mul(inp_word!(1)))),
            _ => None,
        },
        NodeKind::Div => match get_in(n, 0, graph, cache, functions)? {
            NodeValue::Byte(a) => {
                let b = inp_byte!(1);
                if b == 0 { None } else { Some(NodeValue::Byte(a / b)) }
            }
            NodeValue::Word(a) => {
                let b = inp_word!(1);
                if b == 0 { None } else { Some(NodeValue::Word(a / b)) }
            }
            _ => None,
        },
        NodeKind::Mod => match get_in(n, 0, graph, cache, functions)? {
            NodeValue::Byte(a) => {
                let b = inp_byte!(1);
                if b == 0 { None } else { Some(NodeValue::Byte(a % b)) }
            }
            NodeValue::Word(a) => {
                let b = inp_word!(1);
                if b == 0 { None } else { Some(NodeValue::Word(a % b)) }
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
            Some(NodeValue::Byte(((word >> (offset as u32 * 8)) & 0xFF) as u8))
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
            for (src_id, node) in sub.nodes() {
                if matches!(node, NodeKind::Source { .. }) {
                    for out_port in 0..node.output_count() {
                        let val = get_in(n, arg_idx, graph, cache, functions);
                        sub_cache.insert(OutPinId { node: src_id, output: out_port }, val);
                        arg_idx += 1;
                    }
                }
            }

            // Evaluate the Sink's o-th input in the subgraph.
            for (sink_id, node) in sub.nodes() {
                if matches!(node, NodeKind::Sink) {
                    if let Some(src) =
                        sub.sources_of(InPinId { node: sink_id, input: o }).next()
                    {
                        return eval_output(src, sub, &mut sub_cache, functions);
                    }
                }
            }
            None
        }

        _ => None, // Sink/Source have no computed outputs
    }
}
