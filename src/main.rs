use std::path::PathBuf;

type GraphId = u64;

pub enum Value {
	Bytes(Vec<u8>),
	Int(i64),
	Float(f64),
	Bool(bool),
	Text(String),
	List(Vec<Value>),
	Map(Vec<(Value, Value)>),
	Null,
}

pub enum NodeKind {
	// Primitives
	Literal(Value),
	FileSink { path: PathBuf },
	FileSource { path: PathBuf },

	// Byte manipulation
	Concat,
	Slice { start: usize, len: Option<usize> },
	EncodeU32,
	DecodeU32,

	// Control flow
	Map,    // apply subgraph to list
	Fold,   // reduce list with subgraph
	Branch, // conditional

	// Introspection (enables self-hosting)
	GraphToList,
	ListToGraph,

	// Composition
	Subgraph(GraphId), // reference to another graph by ID
}

fn main() -> eframe::Result {
	let options = eframe::NativeOptions::default();
	eframe::run_native(
		"Node Compiler",
		options,
		Box::new(|_cc| Ok(Box::new(App::default()))),
	)
}

#[derive(Default)]
struct App;

impl eframe::App for App {
	fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
		ui.heading("Node Compiler");
	}
}
