use std::{collections::HashMap, path::PathBuf};

use eframe::CreationContext;
use egui::Id;
use egui_snarl::{
	InPin, InPinId, OutPin, OutPinId, Snarl,
	ui::{AnyPins, PinInfo, SnarlPin, SnarlStyle, SnarlViewer, SnarlWidget},
};

// ─── Wire types ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

	fn pin_info(self) -> PinInfo {
		PinInfo::circle()
			.with_fill(self.color())
			.with_wire_color(self.color())
	}

	fn label(self) -> &'static str {
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

#[derive(Clone, Debug)]
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

	fn short_display(&self) -> String {
		match self {
			NodeValue::Bit(v) => {
				if *v {
					"1".into()
				} else {
					"0".into()
				}
			}
			NodeValue::Byte(v) => format!("{:#04x}", v),
			NodeValue::Word(v) => format!("{:#018x}", v),
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

type EvalCache = HashMap<OutPinId, Option<NodeValue>>;

/// Evaluate every output pin in the graph. Returns a cache keyed by OutPinId.
fn eval_graph(snarl: &Snarl<NodeKind>, functions: &[FunctionDef]) -> EvalCache {
	let mut cache = EvalCache::new();
	for (node_id, node) in snarl.node_ids() {
		let n_out = node.output_count();
		for out in 0..n_out {
			eval_output(
				OutPinId { node: node_id, output: out },
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
				if b == 0 { None } else { Some(NodeValue::Byte(a / b)) }
			}
			NodeValue::Word(a) => {
				let b = inp_word!(1);
				if b == 0 { None } else { Some(NodeValue::Word(a / b)) }
			}
			_ => None,
		},
		NodeKind::Mod => match get_in(n, 0, snarl, cache, functions)? {
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
						sub_cache.insert(OutPinId { node: src_id, output: out_port }, val);
						arg_idx += 1;
					}
				}
			}

			// Evaluate the Sink's o-th input in the subgraph.
			for (sink_id, node) in sub.node_ids() {
				if matches!(node, NodeKind::Sink) {
					let in_pin = sub.in_pin(InPinId { node: sink_id, input: o });
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

#[derive(Clone, Debug)]
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
			NodeKind::Source { filename: "args".into() },
		);
		graph.insert_node(egui::pos2(200.0, 0.0), NodeKind::Sink);
		FunctionDef { name: name.into(), graph }
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

#[derive(Clone, Debug)]
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
	fn node_title(&self) -> String {
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

	fn input_count(&self) -> usize {
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

	fn output_count(&self) -> usize {
		match self {
			NodeKind::Source { .. } => 1,
			NodeKind::Sink => 0,
			NodeKind::Constant(vals) => vals.len(),
			NodeKind::Unpack => 8,
			NodeKind::FunctionCall { out_types, .. } => out_types.len(),
			_ => 1,
		}
	}

	fn input_wire_type(&self, port: usize) -> WireType {
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

	fn output_wire_type(&self, port: usize) -> WireType {
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

	fn input_label(&self, port: usize) -> String {
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

// ─── App ─────────────────────────────────────────────────────────────────────

pub struct App {
	snarl: Snarl<NodeKind>,
	style: SnarlStyle,
	functions: Vec<FunctionDef>,
	editing: Option<usize>, // Some(idx) = currently editing functions[idx].graph
}

impl App {
	pub fn new(_cx: &CreationContext) -> Self {
		let mut snarl = Snarl::new();

		let lit = snarl.insert_node(
			egui::pos2(-200.0, 0.0),
			NodeKind::Source {
				filename: "".into(),
			},
		);
		let sink = snarl.insert_node(egui::pos2(100.0, 0.0), NodeKind::Sink);
		snarl.connect(
			OutPinId {
				node: lit,
				output: 0,
			},
			InPinId {
				node: sink,
				input: 0,
			},
		);

		Self {
			snarl,
			style: SnarlStyle::default(),
			functions: Vec::new(),
			editing: None,
		}
	}
}

impl eframe::App for App {
	fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
		// Snapshot function signatures to pass to the viewer without borrow conflicts.
		// Types are derived live from each subgraph's Source/Sink nodes.
		let fn_sigs: Vec<(String, Vec<WireType>, Vec<WireType>)> = self
			.functions
			.iter()
			.map(|f| {
				let (in_types, out_types) = f.call_types();
				(f.display_name(), in_types, out_types)
			})
			.collect();

		egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
			egui::MenuBar::new().ui(ui, |ui| {
				ui.menu_button("File", |ui| {
					if ui.button("Quit").clicked() {
						ctx.send_viewport_cmd(egui::ViewportCommand::Close);
					}
				});
				ui.add_space(16.0);
			});
		});

		// Breadcrumb bar when editing a function subgraph.
		if let Some(idx) = self.editing {
			egui::TopBottomPanel::top("breadcrumb").show(ctx, |ui| {
				ui.horizontal(|ui| {
					if ui.button("< Root").clicked() {
						self.editing = None;
					}
					ui.label("›");
					ui.text_edit_singleline(&mut self.functions[idx].name);
				});
			});
		}

		// Sidebar: function library.
		egui::SidePanel::left("function_sidebar").show(ctx, |ui| {
			ui.heading("Functions");
			if ui.button("+ New").clicked() {
				self.functions.push(FunctionDef::new("Unnamed"));
				self.editing = Some(self.functions.len() - 1);
			}
			ui.separator();

			let n = self.functions.len();
			let mut to_delete: Option<usize> = None;
			let mut to_edit: Option<usize> = None;
			let mut to_add: Option<usize> = None;

			for i in 0..n {
				ui.horizontal(|ui| {
					ui.label(&self.functions[i].name);
					if ui.small_button("Edit").clicked() {
						to_edit = Some(i);
					}
					if ui.small_button("Add").clicked() {
						to_add = Some(i);
					}
					if ui.small_button("Delete").clicked() {
						to_delete = Some(i);
					}
				});
			}

			if let Some(i) = to_edit {
				self.editing = Some(i);
			}
			if let Some(i) = to_add {
				let (in_types, out_types) = self.functions[i].call_types();
				let node = NodeKind::FunctionCall {
					def_index: i,
					name: self.functions[i].name.clone(),
					in_types,
					out_types,
				};
				self.snarl.insert_node(egui::pos2(0.0, 0.0), node);
			}
			if let Some(i) = to_delete {
				self.functions.remove(i);
				// TODO: fix stale def_index in FunctionCall nodes after deletion
				if self.editing == Some(i) {
					self.editing = None;
				}
			}
		});

		egui::CentralPanel::default().show(ctx, |ui| match self.editing {
			None => {
				let cache = eval_graph(&self.snarl, &self.functions);
				SnarlWidget::new()
					.id(Id::new("root_snarl"))
					.style(self.style)
					.show(
						&mut self.snarl,
						&mut NodeGraphViewer {
							cache: &cache,
							fn_sigs: &fn_sigs,
							in_subgraph: false,
						},
						ui,
					);
			}
			Some(idx) => {
				let cache = eval_graph(&self.functions[idx].graph, &self.functions);
				SnarlWidget::new()
					.id(Id::new(("fn_snarl", idx)))
					.style(self.style)
					.show(
						&mut self.functions[idx].graph,
						&mut NodeGraphViewer {
							cache: &cache,
							fn_sigs: &fn_sigs,
							in_subgraph: true,
						},
						ui,
					);
			}
		});
	}
}

// ─── Viewer ──────────────────────────────────────────────────────────────────

struct NodeGraphViewer<'a> {
	cache: &'a EvalCache,
	fn_sigs: &'a [(String, Vec<WireType>, Vec<WireType>)],
	in_subgraph: bool,
}

impl NodeGraphViewer<'_> {
	/// Render the cached value for an output pin as dim monospace text.
	fn show_value(&self, pin: OutPinId, ui: &mut egui::Ui) {
		if let Some(Some(val)) = self.cache.get(&pin) {
			ui.label(
				egui::RichText::new(val.short_display())
					.weak()
					.monospace()
					.small(),
			);
		}
	}
}

impl SnarlViewer<NodeKind> for NodeGraphViewer<'_> {
	// ── Required ──────────────────────────────────────────────────────────

	fn title(&mut self, node: &NodeKind) -> String {
		match node {
			// Look up live display name (includes hash for anonymous functions).
			NodeKind::FunctionCall { def_index, name, .. } => self
				.fn_sigs
				.get(*def_index)
				.map(|(n, _, _)| n.clone())
				.unwrap_or_else(|| name.clone()),
			_ => node.node_title(),
		}
	}

	fn inputs(&mut self, node: &NodeKind) -> usize {
		node.input_count()
	}

	fn outputs(&mut self, node: &NodeKind) -> usize {
		node.output_count()
	}

	fn show_input(
		&mut self,
		pin: &InPin,
		ui: &mut egui::Ui,
		snarl: &mut Snarl<NodeKind>,
	) -> impl SnarlPin + 'static {
		let wt = snarl[pin.id.node].input_wire_type(pin.id.input);
		let label = snarl[pin.id.node].input_label(pin.id.input);
		ui.label(label);
		wt.pin_info()
	}

	fn show_output(
		&mut self,
		pin: &OutPin,
		ui: &mut egui::Ui,
		snarl: &mut Snarl<NodeKind>,
	) -> impl SnarlPin + 'static {
		let wt = snarl[pin.id.node].output_wire_type(pin.id.output);
		let port = pin.id.output;

		match &mut snarl[pin.id.node] {
			// Constant: each port has one editable value widget.
			NodeKind::Constant(values) => {
				if let Some(val) = values.get_mut(port) {
					match val {
						NodeValue::Bit(v) => {
							ui.checkbox(v, "");
						}
						NodeValue::Byte(v) => {
							ui.add(egui::DragValue::new(v).hexadecimal(2, false, true));
						}
						NodeValue::Word(v) => {
							ui.add(egui::DragValue::new(v).hexadecimal(16, false, true));
						}
						NodeValue::Bytes(_) => {
							ui.label("(bytes)");
						}
					}
				}
			}

			// Computed nodes: label + cached value.
			NodeKind::Unpack => {
				ui.label(format!("bit[{port}]"));
				self.show_value(pin.id, ui);
			}
			NodeKind::FunctionCall { out_types, .. } => {
				ui.label(format!(
					"out[{port}] ({})",
					out_types.get(port).unwrap_or(&WireType::Byte).label()
				));
				self.show_value(pin.id, ui);
			}
			_ => {
				ui.label("out");
				self.show_value(pin.id, ui);
			}
		}

		wt.pin_info()
	}

	// ── Node body — used for the Sink to show its incoming value ──────────

	fn has_body(&mut self, node: &NodeKind) -> bool {
		matches!(node, NodeKind::Sink)
	}

	fn show_body(
		&mut self,
		_node: egui_snarl::NodeId,
		inputs: &[InPin],
		_outputs: &[OutPin],
		ui: &mut egui::Ui,
		_snarl: &mut Snarl<NodeKind>,
	) {
		match inputs.first().and_then(|p| p.remotes.first()) {
			Some(src) => match self.cache.get(src) {
				Some(Some(val)) => {
					ui.label(egui::RichText::new(val.short_display()).monospace());
				}
				_ => {
					ui.label(egui::RichText::new("?").weak());
				}
			},
			None => {
				ui.label(egui::RichText::new("(unconnected)").weak().italics());
			}
		}
	}

	// ── Node footer — used for the Constant add-output buttons ───────────

	fn has_footer(&mut self, node: &NodeKind) -> bool {
		matches!(node, NodeKind::Constant(_))
	}

	fn show_footer(
		&mut self,
		node: egui_snarl::NodeId,
		_inputs: &[InPin],
		_outputs: &[OutPin],
		ui: &mut egui::Ui,
		snarl: &mut Snarl<NodeKind>,
	) {
		if let NodeKind::Constant(values) = &mut snarl[node] {
			ui.horizontal(|ui| {
				if ui.small_button("+ Bit").clicked() {
					values.push(NodeValue::Bit(false));
				}
				if ui.small_button("+ Byte").clicked() {
					values.push(NodeValue::Byte(0));
				}
				if ui.small_button("+ Word").clicked() {
					values.push(NodeValue::Word(0));
				}
			});
		}
	}

	// ── Wire connection ───────────────────────────────────────────────────

	fn connect(&mut self, from: &OutPin, to: &InPin, snarl: &mut Snarl<NodeKind>) {
		let from_type = snarl[from.id.node].output_wire_type(from.id.output);
		let to_type = snarl[to.id.node].input_wire_type(to.id.input);

		if from_type != to_type {
			return;
		}

		for &remote in &to.remotes {
			snarl.disconnect(remote, to.id);
		}
		snarl.connect(from.id, to.id);
	}

	// ── Context menus ─────────────────────────────────────────────────────

	fn has_graph_menu(&mut self, _pos: egui::Pos2, _snarl: &mut Snarl<NodeKind>) -> bool {
		true
	}

	fn show_graph_menu(&mut self, pos: egui::Pos2, ui: &mut egui::Ui, snarl: &mut Snarl<NodeKind>) {
		ui.label("Add node:");
		ui.separator();

		if ui.button("Constant").clicked() {
			snarl.insert_node(pos, NodeKind::Constant(Vec::new()));
			ui.close();
		}

		ui.separator();

		ui.menu_button("Bitwise", |ui| {
			if ui.button("AND").clicked() {
				snarl.insert_node(pos, NodeKind::And);
				ui.close();
			}
			if ui.button("OR").clicked() {
				snarl.insert_node(pos, NodeKind::Or);
				ui.close();
			}
			if ui.button("XOR").clicked() {
				snarl.insert_node(pos, NodeKind::Xor);
				ui.close();
			}
			if ui.button("NOT").clicked() {
				snarl.insert_node(pos, NodeKind::Not);
				ui.close();
			}
			if ui.button("NAND").clicked() {
				snarl.insert_node(pos, NodeKind::Nand);
				ui.close();
			}
			if ui.button("NOR").clicked() {
				snarl.insert_node(pos, NodeKind::Nor);
				ui.close();
			}
			if ui.button("SHL").clicked() {
				snarl.insert_node(pos, NodeKind::Shl);
				ui.close();
			}
			if ui.button("SHR").clicked() {
				snarl.insert_node(pos, NodeKind::Shr);
				ui.close();
			}
		});

		ui.menu_button("Arithmetic", |ui| {
			if ui.button("ADD").clicked() {
				snarl.insert_node(pos, NodeKind::Add);
				ui.close();
			}
			if ui.button("SUB").clicked() {
				snarl.insert_node(pos, NodeKind::Sub);
				ui.close();
			}
			if ui.button("MUL").clicked() {
				snarl.insert_node(pos, NodeKind::Mul);
				ui.close();
			}
			if ui.button("DIV").clicked() {
				snarl.insert_node(pos, NodeKind::Div);
				ui.close();
			}
			if ui.button("MOD").clicked() {
				snarl.insert_node(pos, NodeKind::Mod);
				ui.close();
			}
		});

		ui.menu_button("Byte manipulation", |ui| {
			if ui.button("CONCAT (2)").clicked() {
				snarl.insert_node(pos, NodeKind::Concat { count: 2 });
				ui.close();
			}
			if ui.button("CONCAT (4)").clicked() {
				snarl.insert_node(pos, NodeKind::Concat { count: 4 });
				ui.close();
			}
			if ui.button("SLICE").clicked() {
				snarl.insert_node(pos, NodeKind::Slice);
				ui.close();
			}
			if ui.button("PACK").clicked() {
				snarl.insert_node(pos, NodeKind::Pack);
				ui.close();
			}
			if ui.button("UNPACK").clicked() {
				snarl.insert_node(pos, NodeKind::Unpack);
				ui.close();
			}
		});

		if !self.fn_sigs.is_empty() {
			ui.separator();
			ui.menu_button("Call function", |ui| {
				for (i, (name, in_types, out_types)) in self.fn_sigs.iter().enumerate() {
					if ui.button(name).clicked() {
						snarl.insert_node(
							pos,
							NodeKind::FunctionCall {
								def_index: i,
								name: name.clone(),
								in_types: in_types.clone(),
								out_types: out_types.clone(),
							},
						);
						ui.close();
					}
				}
			});
		}

		if !self.in_subgraph {
			ui.separator();
			if ui.button("SINK").clicked() {
				snarl.insert_node(pos, NodeKind::Sink);
				ui.close();
			}
		}
	}

	fn has_dropped_wire_menu(&mut self, _src_pins: AnyPins, _snarl: &mut Snarl<NodeKind>) -> bool {
		true
	}

	fn show_dropped_wire_menu(
		&mut self,
		pos: egui::Pos2,
		ui: &mut egui::Ui,
		src_pins: AnyPins,
		snarl: &mut Snarl<NodeKind>,
	) {
		let wt = match src_pins {
			AnyPins::Out(ids) => ids
				.first()
				.map(|id| snarl[id.node].output_wire_type(id.output)),
			AnyPins::In(ids) => ids
				.first()
				.map(|id| snarl[id.node].input_wire_type(id.input)),
		};

		ui.label(match wt {
			Some(WireType::Bit) => "Bit wire — add node:",
			Some(WireType::Word) => "Word wire — add node:",
			_ => "Byte wire — add node:",
		});
		ui.separator();

		match wt {
            Some(WireType::Bit) => {
                if ui.button("AND").clicked()  { snarl.insert_node(pos, NodeKind::And);  ui.close(); }
                if ui.button("OR").clicked()   { snarl.insert_node(pos, NodeKind::Or);   ui.close(); }
                if ui.button("NOT").clicked()  { snarl.insert_node(pos, NodeKind::Not);  ui.close(); }
                if ui.button("PACK").clicked() { snarl.insert_node(pos, NodeKind::Pack); ui.close(); }
            }
            Some(WireType::Word) => {
                if ui.button("ADD").clicked()           { snarl.insert_node(pos, NodeKind::Add);   ui.close(); }
                if ui.button("SUB").clicked()           { snarl.insert_node(pos, NodeKind::Sub);   ui.close(); }
                if ui.button("AND").clicked()           { snarl.insert_node(pos, NodeKind::And);   ui.close(); }
                if ui.button("SLICE").clicked()         { snarl.insert_node(pos, NodeKind::Slice); ui.close(); }
            }
            _ /* Byte */ => {
                if ui.button("AND").clicked()             { snarl.insert_node(pos, NodeKind::And);  ui.close(); }
                if ui.button("CONCAT (2)").clicked()      { snarl.insert_node(pos, NodeKind::Concat { count: 2 }); ui.close(); }
                if ui.button("UNPACK").clicked()          { snarl.insert_node(pos, NodeKind::Unpack); ui.close(); }
                if !self.in_subgraph && ui.button("SINK").clicked() { snarl.insert_node(pos, NodeKind::Sink); ui.close(); }
            }
        }
	}

	fn has_node_menu(&mut self, _node: &NodeKind) -> bool {
		true
	}

	fn show_node_menu(
		&mut self,
		node: egui_snarl::NodeId,
		_inputs: &[InPin],
		_outputs: &[OutPin],
		ui: &mut egui::Ui,
		snarl: &mut Snarl<NodeKind>,
	) {
		// Constant-specific controls: add/remove output ports.
		{
			if let NodeKind::Constant(values) = &mut snarl[node] {
				ui.menu_button("Add output", |ui| {
					if ui.button("Bit").clicked() {
						values.push(NodeValue::Bit(false));
						ui.close();
					}
					if ui.button("Byte").clicked() {
						values.push(NodeValue::Byte(0));
						ui.close();
					}
					if ui.button("Word").clicked() {
						values.push(NodeValue::Word(0));
						ui.close();
					}
				});
				if !values.is_empty() && ui.button("Remove last output").clicked() {
					values.pop();
					ui.close();
				}
				ui.separator();
			}
		}

		if ui.button("Delete node").clicked() {
			snarl.remove_node(node);
			ui.close();
		}
	}
}
