use eframe::CreationContext;
use egui::Id;
use egui_snarl::{
    InPin, OutPin, Snarl,
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
            WireType::Bit  => egui::Color32::from_rgb(255, 200,  50), // yellow
            WireType::Byte => egui::Color32::from_rgb(100, 200, 255), // sky blue
            WireType::Word => egui::Color32::from_rgb(180, 100, 255), // purple
        }
    }

    fn pin_info(self) -> PinInfo {
        PinInfo::circle().with_fill(self.color()).with_wire_color(self.color())
    }

    fn label(self) -> &'static str {
        match self { WireType::Bit => "Bit", WireType::Byte => "Byte", WireType::Word => "Word" }
    }
}

// ─── Node kinds (mirrors spec discriminant catalogue) ────────────────────────

#[derive(Clone, Debug)]
pub enum NodeKind {
    // Structural (0x0001–0x00FF)
    Sink,

    // Literals (0x0100–0x01FF)
    LitBit(bool),
    LitByte(u8),
    LitWord(u64),

    // Bitwise — monomorphized per type (0x0200–0x02FF)
    AndBit,  AndByte,  AndWord,
    OrBit,   OrByte,   OrWord,
    XorBit,  XorByte,  XorWord,
    NotBit,  NotByte,  NotWord,
    NandBit, NandByte, NandWord,
    NorBit,  NorByte,  NorWord,
    ShlByte, ShlWord,
    ShrByte, ShrWord,

    // Arithmetic — all Word (0x0300–0x03FF)
    AddWord, SubWord, MulWord, DivWord, ModWord,

    // Byte manipulation (0x0400–0x04FF)
    Concat { count: u8 },
    Slice,
    Pack,
    Unpack,

    // Scoping (0x0500–0x05FF)
    Function { name: String, in_types: Vec<WireType>, out_types: Vec<WireType> },
    Module   { name: String },
    Record   { name: String, field_types: Vec<WireType> },

    // Binary generation (0x0600–0x06FF)
    Instruction  { operand_count: u8 },
    Label        { name: String },
    BranchTarget { relative: bool },
    ElfHeader,
    PeHeader,
}

impl NodeKind {
    fn node_title(&self) -> String {
        match self {
            NodeKind::Sink         => "SINK".into(),
            NodeKind::LitBit(_)   => "LIT BIT".into(),
            NodeKind::LitByte(_)  => "LIT BYTE".into(),
            NodeKind::LitWord(_)  => "LIT WORD".into(),
            NodeKind::AndBit      => "AND (Bit)".into(),
            NodeKind::AndByte     => "AND (Byte)".into(),
            NodeKind::AndWord     => "AND (Word)".into(),
            NodeKind::OrBit       => "OR (Bit)".into(),
            NodeKind::OrByte      => "OR (Byte)".into(),
            NodeKind::OrWord      => "OR (Word)".into(),
            NodeKind::XorBit      => "XOR (Bit)".into(),
            NodeKind::XorByte     => "XOR (Byte)".into(),
            NodeKind::XorWord     => "XOR (Word)".into(),
            NodeKind::NotBit      => "NOT (Bit)".into(),
            NodeKind::NotByte     => "NOT (Byte)".into(),
            NodeKind::NotWord     => "NOT (Word)".into(),
            NodeKind::NandBit     => "NAND (Bit)".into(),
            NodeKind::NandByte    => "NAND (Byte)".into(),
            NodeKind::NandWord    => "NAND (Word)".into(),
            NodeKind::NorBit      => "NOR (Bit)".into(),
            NodeKind::NorByte     => "NOR (Byte)".into(),
            NodeKind::NorWord     => "NOR (Word)".into(),
            NodeKind::ShlByte     => "SHL (Byte)".into(),
            NodeKind::ShlWord     => "SHL (Word)".into(),
            NodeKind::ShrByte     => "SHR (Byte)".into(),
            NodeKind::ShrWord     => "SHR (Word)".into(),
            NodeKind::AddWord     => "ADD".into(),
            NodeKind::SubWord     => "SUB".into(),
            NodeKind::MulWord     => "MUL".into(),
            NodeKind::DivWord     => "DIV".into(),
            NodeKind::ModWord     => "MOD".into(),
            NodeKind::Concat { count } => format!("CONCAT ({})", count),
            NodeKind::Slice       => "SLICE".into(),
            NodeKind::Pack        => "PACK".into(),
            NodeKind::Unpack      => "UNPACK".into(),
            NodeKind::Function { name, .. } => name.clone(),
            NodeKind::Module   { name }     => name.clone(),
            NodeKind::Record   { name, .. } => name.clone(),
            NodeKind::Instruction { operand_count } => format!("INSTR ({})", operand_count),
            NodeKind::Label        { name } => name.clone(),
            NodeKind::BranchTarget { relative } =>
                if *relative { "BRANCH REL".into() } else { "BRANCH ABS".into() },
            NodeKind::ElfHeader   => "ELF HEADER".into(),
            NodeKind::PeHeader    => "PE HEADER".into(),
        }
    }

    fn input_count(&self) -> usize {
        match self {
            NodeKind::Sink                                   => 1,
            NodeKind::LitBit(_) | NodeKind::LitByte(_)
            | NodeKind::LitWord(_) | NodeKind::Label { .. }
            | NodeKind::Module { .. }                        => 0,
            NodeKind::NotBit | NodeKind::NotByte | NodeKind::NotWord
            | NodeKind::Unpack                               => 1,
            NodeKind::AndBit  | NodeKind::AndByte  | NodeKind::AndWord
            | NodeKind::OrBit  | NodeKind::OrByte  | NodeKind::OrWord
            | NodeKind::XorBit | NodeKind::XorByte | NodeKind::XorWord
            | NodeKind::NandBit| NodeKind::NandByte| NodeKind::NandWord
            | NodeKind::NorBit | NodeKind::NorByte | NodeKind::NorWord
            | NodeKind::ShlByte| NodeKind::ShlWord
            | NodeKind::ShrByte| NodeKind::ShrWord
            | NodeKind::AddWord| NodeKind::SubWord | NodeKind::MulWord
            | NodeKind::DivWord| NodeKind::ModWord | NodeKind::Slice
            | NodeKind::BranchTarget { .. } | NodeKind::PeHeader    => 2,
            NodeKind::Pack                                   => 8,
            NodeKind::ElfHeader                              => 3,
            NodeKind::Concat { count }                       => *count as usize,
            NodeKind::Function { in_types, .. }              => in_types.len(),
            NodeKind::Record   { field_types, .. }           => field_types.len(),
            NodeKind::Instruction { operand_count }          => 1 + *operand_count as usize,
        }
    }

    fn output_count(&self) -> usize {
        match self {
            NodeKind::Sink | NodeKind::Module { .. }   => 0,
            NodeKind::Unpack                           => 8,
            NodeKind::Function { out_types, .. }       => out_types.len(),
            _                                          => 1,
        }
    }

    fn input_wire_type(&self, port: usize) -> WireType {
        match self {
            NodeKind::Sink | NodeKind::Concat { .. }   => WireType::Byte,
            NodeKind::AndBit  | NodeKind::OrBit  | NodeKind::XorBit
            | NodeKind::NotBit | NodeKind::NandBit | NodeKind::NorBit => WireType::Bit,
            NodeKind::AndWord | NodeKind::OrWord | NodeKind::XorWord
            | NodeKind::NotWord | NodeKind::NandWord | NodeKind::NorWord => WireType::Word,
            NodeKind::ShlByte | NodeKind::ShrByte => WireType::Byte,
            NodeKind::ShlWord | NodeKind::ShrWord =>
                if port == 0 { WireType::Word } else { WireType::Byte },
            NodeKind::AddWord | NodeKind::SubWord | NodeKind::MulWord
            | NodeKind::DivWord | NodeKind::ModWord => WireType::Word,
            NodeKind::Slice =>
                if port == 0 { WireType::Word } else { WireType::Byte },
            NodeKind::Pack     => WireType::Bit,
            NodeKind::Unpack   => WireType::Byte,
            NodeKind::Function { in_types, .. } =>
                in_types.get(port).copied().unwrap_or(WireType::Byte),
            NodeKind::Record { field_types, .. } =>
                field_types.get(port).copied().unwrap_or(WireType::Byte),
            NodeKind::BranchTarget { .. } | NodeKind::ElfHeader | NodeKind::PeHeader => WireType::Word,
            NodeKind::Instruction { .. } => WireType::Byte,
            _ => WireType::Byte,
        }
    }

    fn output_wire_type(&self, port: usize) -> WireType {
        match self {
            NodeKind::LitBit(_)   => WireType::Bit,
            NodeKind::LitWord(_)  => WireType::Word,
            NodeKind::AndBit  | NodeKind::OrBit  | NodeKind::XorBit
            | NodeKind::NotBit | NodeKind::NandBit | NodeKind::NorBit => WireType::Bit,
            NodeKind::AndWord | NodeKind::OrWord | NodeKind::XorWord
            | NodeKind::NotWord | NodeKind::NandWord | NodeKind::NorWord => WireType::Word,
            NodeKind::ShlWord | NodeKind::ShrWord => WireType::Word,
            NodeKind::AddWord | NodeKind::SubWord | NodeKind::MulWord
            | NodeKind::DivWord | NodeKind::ModWord => WireType::Word,
            NodeKind::Unpack  => WireType::Bit,
            NodeKind::Label { .. } => WireType::Word,
            NodeKind::Function { out_types, .. } =>
                out_types.get(port).copied().unwrap_or(WireType::Byte),
            _ => WireType::Byte,
        }
    }

    fn input_label(&self, port: usize) -> String {
        match self {
            NodeKind::Sink => "in".into(),
            NodeKind::AndBit  | NodeKind::AndByte  | NodeKind::AndWord
            | NodeKind::OrBit  | NodeKind::OrByte  | NodeKind::OrWord
            | NodeKind::XorBit | NodeKind::XorByte | NodeKind::XorWord
            | NodeKind::NandBit| NodeKind::NandByte| NodeKind::NandWord
            | NodeKind::NorBit | NodeKind::NorByte | NodeKind::NorWord
            | NodeKind::AddWord| NodeKind::SubWord | NodeKind::MulWord
            | NodeKind::DivWord| NodeKind::ModWord =>
                if port == 0 { "a".into() } else { "b".into() },
            NodeKind::NotBit | NodeKind::NotByte | NodeKind::NotWord => "a".into(),
            NodeKind::ShlByte | NodeKind::ShlWord | NodeKind::ShrByte | NodeKind::ShrWord =>
                if port == 0 { "a".into() } else { "amount".into() },
            NodeKind::Concat { .. } => format!("in[{port}]"),
            NodeKind::Slice =>
                if port == 0 { "in".into() } else { "offset".into() },
            NodeKind::Pack    => format!("bit[{port}]"),
            NodeKind::Unpack  => "in".into(),
            NodeKind::Function { .. } => format!("in[{port}]"),
            NodeKind::Record { .. }   => format!("field[{port}]"),
            NodeKind::Instruction { .. } =>
                if port == 0 { "opcode".into() } else { format!("op[{}]", port - 1) },
            NodeKind::BranchTarget { .. } =>
                if port == 0 { "target".into() } else { "base".into() },
            NodeKind::ElfHeader => match port {
                0 => "entry".into(), 1 => "text_off".into(), _ => "text_size".into(),
            },
            NodeKind::PeHeader =>
                if port == 0 { "entry".into() } else { "img_base".into() },
            _ => format!("in[{port}]"),
        }
    }

}

// ─── App ─────────────────────────────────────────────────────────────────────

struct App {
    snarl: Snarl<NodeKind>,
    style: SnarlStyle,
}

impl App {
    fn new(_cx: &CreationContext) -> Self {
        let mut snarl = Snarl::new();

        // Seed a small demo graph so there's something to look at on launch.
        let lit = snarl.insert_node(egui::pos2(-200.0, 0.0),   NodeKind::LitByte(0xAB));
        let sink = snarl.insert_node(egui::pos2(100.0, 0.0),   NodeKind::Sink);
        snarl.connect(
            egui_snarl::OutPinId { node: lit,  output: 0 },
            egui_snarl::InPinId  { node: sink, input:  0 },
        );

        Self { snarl, style: SnarlStyle::default() }
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
            });
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            SnarlWidget::new()
                .id(Id::new("snarl"))
                .style(self.style)
                .show(&mut self.snarl, &mut NodeGraphViewer, ui);
        });
    }
}

// ─── Viewer ──────────────────────────────────────────────────────────────────

struct NodeGraphViewer;

impl SnarlViewer<NodeKind> for NodeGraphViewer {
    // ── Required ──────────────────────────────────────────────────────────

    fn title(&mut self, node: &NodeKind) -> String {
        node.node_title()
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
        let wt    = snarl[pin.id.node].input_wire_type(pin.id.input);
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
        let wt   = snarl[pin.id.node].output_wire_type(pin.id.output);
        let port = pin.id.output;

        match &mut snarl[pin.id.node] {
            NodeKind::LitBit(v)  => { ui.checkbox(v, ""); }
            NodeKind::LitByte(v) => { ui.add(egui::DragValue::new(v).hexadecimal(2, false, true)); }
            NodeKind::LitWord(v) => { ui.add(egui::DragValue::new(v).hexadecimal(16, false, true)); }
            NodeKind::Unpack     => { ui.label(format!("bit[{port}]")); }
            NodeKind::Label { name } => { ui.label(name.as_str()); }
            NodeKind::Function { out_types, .. } => {
                let label = format!(
                    "out[{port}] ({})",
                    out_types.get(port).unwrap_or(&WireType::Byte).label()
                );
                ui.label(label);
            }
            _ => { ui.label("out"); }
        }

        wt.pin_info()
    }

    // ── Wire connection ───────────────────────────────────────────────────

    fn connect(&mut self, from: &OutPin, to: &InPin, snarl: &mut Snarl<NodeKind>) {
        let from_type = snarl[from.id.node].output_wire_type(from.id.output);
        let to_type   = snarl[to.id.node].input_wire_type(to.id.input);

        if from_type != to_type {
            return; // type mismatch — reject silently
        }

        // Enforce single-source per input pin.
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
        ui.label("Add node");
        ui.separator();

        ui.menu_button("Literals", |ui| {
            if ui.button("Bit").clicked()  { snarl.insert_node(pos, NodeKind::LitBit(false)); ui.close(); }
            if ui.button("Byte").clicked() { snarl.insert_node(pos, NodeKind::LitByte(0));    ui.close(); }
            if ui.button("Word").clicked() { snarl.insert_node(pos, NodeKind::LitWord(0));    ui.close(); }
        });

        ui.menu_button("Bitwise", |ui| {
            ui.menu_button("AND", |ui| {
                if ui.button("Bit").clicked()  { snarl.insert_node(pos, NodeKind::AndBit);  ui.close(); }
                if ui.button("Byte").clicked() { snarl.insert_node(pos, NodeKind::AndByte); ui.close(); }
                if ui.button("Word").clicked() { snarl.insert_node(pos, NodeKind::AndWord); ui.close(); }
            });
            ui.menu_button("OR", |ui| {
                if ui.button("Bit").clicked()  { snarl.insert_node(pos, NodeKind::OrBit);  ui.close(); }
                if ui.button("Byte").clicked() { snarl.insert_node(pos, NodeKind::OrByte); ui.close(); }
                if ui.button("Word").clicked() { snarl.insert_node(pos, NodeKind::OrWord); ui.close(); }
            });
            ui.menu_button("XOR", |ui| {
                if ui.button("Bit").clicked()  { snarl.insert_node(pos, NodeKind::XorBit);  ui.close(); }
                if ui.button("Byte").clicked() { snarl.insert_node(pos, NodeKind::XorByte); ui.close(); }
                if ui.button("Word").clicked() { snarl.insert_node(pos, NodeKind::XorWord); ui.close(); }
            });
            ui.menu_button("NOT", |ui| {
                if ui.button("Bit").clicked()  { snarl.insert_node(pos, NodeKind::NotBit);  ui.close(); }
                if ui.button("Byte").clicked() { snarl.insert_node(pos, NodeKind::NotByte); ui.close(); }
                if ui.button("Word").clicked() { snarl.insert_node(pos, NodeKind::NotWord); ui.close(); }
            });
            ui.menu_button("NAND", |ui| {
                if ui.button("Bit").clicked()  { snarl.insert_node(pos, NodeKind::NandBit);  ui.close(); }
                if ui.button("Byte").clicked() { snarl.insert_node(pos, NodeKind::NandByte); ui.close(); }
                if ui.button("Word").clicked() { snarl.insert_node(pos, NodeKind::NandWord); ui.close(); }
            });
            ui.menu_button("NOR", |ui| {
                if ui.button("Bit").clicked()  { snarl.insert_node(pos, NodeKind::NorBit);  ui.close(); }
                if ui.button("Byte").clicked() { snarl.insert_node(pos, NodeKind::NorByte); ui.close(); }
                if ui.button("Word").clicked() { snarl.insert_node(pos, NodeKind::NorWord); ui.close(); }
            });
            ui.menu_button("SHL", |ui| {
                if ui.button("Byte").clicked() { snarl.insert_node(pos, NodeKind::ShlByte); ui.close(); }
                if ui.button("Word").clicked() { snarl.insert_node(pos, NodeKind::ShlWord); ui.close(); }
            });
            ui.menu_button("SHR", |ui| {
                if ui.button("Byte").clicked() { snarl.insert_node(pos, NodeKind::ShrByte); ui.close(); }
                if ui.button("Word").clicked() { snarl.insert_node(pos, NodeKind::ShrWord); ui.close(); }
            });
        });

        ui.menu_button("Arithmetic", |ui| {
            if ui.button("ADD").clicked() { snarl.insert_node(pos, NodeKind::AddWord); ui.close(); }
            if ui.button("SUB").clicked() { snarl.insert_node(pos, NodeKind::SubWord); ui.close(); }
            if ui.button("MUL").clicked() { snarl.insert_node(pos, NodeKind::MulWord); ui.close(); }
            if ui.button("DIV").clicked() { snarl.insert_node(pos, NodeKind::DivWord); ui.close(); }
            if ui.button("MOD").clicked() { snarl.insert_node(pos, NodeKind::ModWord); ui.close(); }
        });

        ui.menu_button("Byte manipulation", |ui| {
            if ui.button("CONCAT (2)").clicked() { snarl.insert_node(pos, NodeKind::Concat { count: 2 }); ui.close(); }
            if ui.button("CONCAT (4)").clicked() { snarl.insert_node(pos, NodeKind::Concat { count: 4 }); ui.close(); }
            if ui.button("SLICE").clicked()      { snarl.insert_node(pos, NodeKind::Slice);              ui.close(); }
            if ui.button("PACK").clicked()       { snarl.insert_node(pos, NodeKind::Pack);               ui.close(); }
            if ui.button("UNPACK").clicked()     { snarl.insert_node(pos, NodeKind::Unpack);             ui.close(); }
        });

        ui.menu_button("Binary generation", |ui| {
            if ui.button("INSTRUCTION (1)").clicked() { snarl.insert_node(pos, NodeKind::Instruction { operand_count: 1 }); ui.close(); }
            if ui.button("INSTRUCTION (3)").clicked() { snarl.insert_node(pos, NodeKind::Instruction { operand_count: 3 }); ui.close(); }
            if ui.button("LABEL").clicked()           { snarl.insert_node(pos, NodeKind::Label { name: "label".into() });   ui.close(); }
            if ui.button("BRANCH (rel)").clicked()    { snarl.insert_node(pos, NodeKind::BranchTarget { relative: true });  ui.close(); }
            if ui.button("BRANCH (abs)").clicked()    { snarl.insert_node(pos, NodeKind::BranchTarget { relative: false }); ui.close(); }
            if ui.button("ELF HEADER").clicked()      { snarl.insert_node(pos, NodeKind::ElfHeader);                        ui.close(); }
            if ui.button("PE HEADER").clicked()       { snarl.insert_node(pos, NodeKind::PeHeader);                         ui.close(); }
        });

        ui.separator();
        if ui.button("SINK").clicked() { snarl.insert_node(pos, NodeKind::Sink); ui.close(); }
    }

    fn has_dropped_wire_menu(
        &mut self,
        _src_pins: AnyPins,
        _snarl: &mut Snarl<NodeKind>,
    ) -> bool {
        true
    }

    fn show_dropped_wire_menu(
        &mut self,
        pos: egui::Pos2,
        ui: &mut egui::Ui,
        src_pins: AnyPins,
        snarl: &mut Snarl<NodeKind>,
    ) {
        // Determine the wire type of the source pin so we can offer compatible nodes.
        let wt = match src_pins {
            AnyPins::Out(ids) => ids
                .first()
                .map(|id| snarl[id.node].output_wire_type(id.output)),
            AnyPins::In(ids) => ids
                .first()
                .map(|id| snarl[id.node].input_wire_type(id.input)),
        };

        ui.label(match wt {
            Some(WireType::Bit)  => "Bit wire — add node:",
            Some(WireType::Word) => "Word wire — add node:",
            _                    => "Byte wire — add node:",
        });
        ui.separator();

        // Show a minimal set of compatible nodes depending on type.
        match wt {
            Some(WireType::Bit) => {
                if ui.button("AND (Bit)").clicked()  { snarl.insert_node(pos, NodeKind::AndBit);  ui.close(); }
                if ui.button("OR (Bit)").clicked()   { snarl.insert_node(pos, NodeKind::OrBit);   ui.close(); }
                if ui.button("NOT (Bit)").clicked()  { snarl.insert_node(pos, NodeKind::NotBit);  ui.close(); }
                if ui.button("PACK").clicked()       { snarl.insert_node(pos, NodeKind::Pack);    ui.close(); }
            }
            Some(WireType::Word) => {
                if ui.button("ADD").clicked()        { snarl.insert_node(pos, NodeKind::AddWord); ui.close(); }
                if ui.button("SUB").clicked()        { snarl.insert_node(pos, NodeKind::SubWord); ui.close(); }
                if ui.button("AND (Word)").clicked() { snarl.insert_node(pos, NodeKind::AndWord); ui.close(); }
                if ui.button("SLICE").clicked()      { snarl.insert_node(pos, NodeKind::Slice);   ui.close(); }
                if ui.button("BRANCH TARGET").clicked() { snarl.insert_node(pos, NodeKind::BranchTarget { relative: true }); ui.close(); }
            }
            _ /* Byte */ => {
                if ui.button("AND (Byte)").clicked() { snarl.insert_node(pos, NodeKind::AndByte); ui.close(); }
                if ui.button("CONCAT (2)").clicked() { snarl.insert_node(pos, NodeKind::Concat { count: 2 }); ui.close(); }
                if ui.button("UNPACK").clicked()     { snarl.insert_node(pos, NodeKind::Unpack);  ui.close(); }
                if ui.button("SINK").clicked()       { snarl.insert_node(pos, NodeKind::Sink);    ui.close(); }
                if ui.button("INSTRUCTION (1)").clicked() { snarl.insert_node(pos, NodeKind::Instruction { operand_count: 1 }); ui.close(); }
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
        if ui.button("Delete node").clicked() {
            snarl.remove_node(node);
            ui.close();
        }
    }
}

// ─── Entry point ─────────────────────────────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_min_inner_size([400.0, 300.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Node Compiler",
        native_options,
        Box::new(|cx| Ok(Box::new(App::new(cx)))),
    )
}
