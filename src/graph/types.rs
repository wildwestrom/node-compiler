use std::path::PathBuf;

use serde::{Deserialize, Serialize};

// ─── Wire types ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireType {
    Bit,
    Byte,
    Word,
}

impl WireType {
    pub fn label(self) -> &'static str {
        match self {
            WireType::Bit => "Bit",
            WireType::Byte => "Byte",
            WireType::Word => "Word",
        }
    }
}

// ─── Runtime values ──────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NodeValue {
    Bit(bool),
    Byte(u8),
    Word(u64),
    Bytes(Vec<u8>),
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

    pub(crate) fn as_bit(&self) -> Option<bool> {
        if let NodeValue::Bit(v) = self {
            Some(*v)
        } else {
            None
        }
    }

    pub(crate) fn as_byte(&self) -> Option<u8> {
        if let NodeValue::Byte(v) = self {
            Some(*v)
        } else {
            None
        }
    }

    pub(crate) fn as_word(&self) -> Option<u64> {
        if let NodeValue::Word(v) = self {
            Some(*v)
        } else {
            None
        }
    }

    pub fn short_display(&self) -> String {
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

// ─── Node kinds ──────────────────────────────────────────────────────────────

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
        name: String,
        in_types: Vec<WireType>,
        out_types: Vec<WireType>,
    },
}

impl NodeKind {
    pub fn node_title(&self) -> String {
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

    pub fn input_count(&self) -> usize {
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

    pub fn output_count(&self) -> usize {
        match self {
            NodeKind::Source { .. } => 1,
            NodeKind::Sink => 0,
            NodeKind::Constant(vals) => vals.len(),
            NodeKind::Unpack => 8,
            NodeKind::FunctionCall { out_types, .. } => out_types.len(),
            _ => 1,
        }
    }

    pub fn input_wire_type(&self, port: usize) -> WireType {
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

    pub fn output_wire_type(&self, port: usize) -> WireType {
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

    pub fn input_label(&self, port: usize) -> String {
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
