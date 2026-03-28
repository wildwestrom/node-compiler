use std::path::PathBuf;

use eframe::CreationContext;
use egui::Id;
use egui_snarl::{
	Snarl,
	ui::{PinInfo, SnarlStyle, SnarlViewer, SnarlWidget},
};

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

struct App {
	snarl: Snarl<NodeKind>,
	style: SnarlStyle,
}

impl App {
	fn new(_cx: &CreationContext) -> Self {
		let snarl = Snarl::new();

		let style = SnarlStyle::default();

		Self { snarl, style }
	}
}

impl eframe::App for App {
	fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
		egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
			egui::MenuBar::new().ui(ui, |ui| {
				ui.menu_button("File", |ui| {
					if ui.button("Quit").clicked() {
						ctx.send_viewport_cmd(egui::ViewportCommand::Close);
					}
				});
				ui.add_space(16.0);
			})
		});
		egui::CentralPanel::default().show(ctx, |ui| {
			SnarlWidget::new()
				.id(Id::new("snarl-testing"))
				.style(self.style)
				.show(&mut self.snarl, &mut NodeGraphViewer, ui)
		});
	}
}

struct NodeGraphViewer;

impl SnarlViewer<NodeKind> for NodeGraphViewer {
	fn connect(
		&mut self,
		from: &egui_snarl::OutPin,
		to: &egui_snarl::InPin,
		snarl: &mut Snarl<NodeKind>,
	) {
		todo!()
	}

	fn title(&mut self, node: &NodeKind) -> String {
		todo!()
	}

	fn inputs(&mut self, node: &NodeKind) -> usize {
		todo!()
	}

	fn outputs(&mut self, node: &NodeKind) -> usize {
		todo!()
	}

	fn show_input(
		&mut self,
		pin: &egui_snarl::InPin,
		ui: &mut egui::Ui,
		snarl: &mut Snarl<NodeKind>,
	) -> PinInfo {
		todo!()
	}

	fn show_output(
		&mut self,
		pin: &egui_snarl::OutPin,
		ui: &mut egui::Ui,
		snarl: &mut Snarl<NodeKind>,
	) -> PinInfo {
		todo!()
	}
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
	let native_options = eframe::NativeOptions {
		viewport: egui::ViewportBuilder::default().with_min_inner_size([300.0, 220.0]),
		..Default::default()
	};

	eframe::run_native(
		"Node Compiler",
		native_options,
		Box::new(|cx| Ok(Box::new(App::new(cx)))),
	)
}
