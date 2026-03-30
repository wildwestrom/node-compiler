use egui_snarl::{
    InPin, NodeId, OutPin, OutPinId as SnarlOut, Snarl,
    ui::{AnyPins, PinInfo, SnarlPin, SnarlViewer},
};

use crate::graph::{EvalCache, FunctionDef, NodeKind, NodeValue, OutPinId, WireType};
use crate::ui::persistence::NamesData;

pub(crate) struct NodeGraphViewer<'a> {
    pub(crate) cache: &'a EvalCache,
    pub(crate) functions: &'a [FunctionDef],
    pub(crate) names: &'a mut NamesData,
    /// `Some(hash)` when editing a function subgraph; `None` for the root graph.
    pub(crate) fn_hash: Option<String>,
}

impl NodeGraphViewer<'_> {
    fn fn_name(&self, def_index: usize) -> String {
        self.functions
            .get(def_index)
            .map(|f| {
                let hash = f.graph_hash();
                self.names
                    .functions
                    .get(&hash)
                    .filter(|n| !n.is_empty())
                    .cloned()
                    .unwrap_or_else(|| format!("#{}", &hash[..8]))
            })
            .unwrap_or_else(|| "FUNCTION".into())
    }

    /// Render the cached value for an output pin as dim monospace text.
    pub(crate) fn show_value(&self, pin: SnarlOut, ui: &mut egui::Ui) {
        let our_pin = OutPinId::from(pin);
        if let Some(Some(val)) = self.cache.get(&our_pin) {
            ui.label(
                egui::RichText::new(val.short_display())
                    .weak()
                    .monospace()
                    .small(),
            );
        }
    }
}

fn pin_info_for(wt: WireType) -> PinInfo {
    let c = match wt {
        WireType::Bit => egui::Color32::from_rgb(255, 200, 50),
        WireType::Byte => egui::Color32::from_rgb(100, 200, 255),
        WireType::Word => egui::Color32::from_rgb(180, 100, 255),
    };
    PinInfo::circle().with_fill(c).with_wire_color(c)
}

impl SnarlViewer<NodeKind> for NodeGraphViewer<'_> {
    // ── Required ──────────────────────────────────────────────────────────

    fn title(&mut self, node: &NodeKind) -> String {
        match node {
            NodeKind::FunctionCall { def_index, .. } => self.fn_name(*def_index),
            _ => node.node_title(),
        }
    }

    fn show_header(
        &mut self,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut egui::Ui,
        snarl: &mut Snarl<NodeKind>,
    ) {
        enum Variant {
            FnCall(usize),
            Constant,
            Other(String),
        }
        let variant = match &snarl[node] {
            NodeKind::FunctionCall { def_index, .. } => Variant::FnCall(*def_index),
            NodeKind::Constant(_) => Variant::Constant,
            n => Variant::Other(n.node_title()),
        };
        match variant {
            Variant::FnCall(def_index) => {
                if let Some(f) = self.functions.get(def_index) {
                    let hash = f.graph_hash();
                    let hint = format!("#{}", &hash[..8]);
                    let name = self.names.functions.entry(hash).or_default();
                    egui::TextEdit::singleline(name).hint_text(hint).show(ui);
                }
            }
            Variant::Constant => {
                let node_id = node.0;
                let fn_hash = self.fn_hash.clone();
                let node_names = match fn_hash {
                    None => &mut self.names.root_nodes,
                    Some(hash) => self.names.subgraph_nodes.entry(hash).or_default(),
                };
                let name = node_names.entry(node_id).or_default();
                egui::TextEdit::singleline(name).hint_text("CONSTANT").show(ui);
            }
            Variant::Other(title) => {
                ui.label(title);
            }
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
        pin_info_for(wt)
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

        pin_info_for(wt)
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
            Some(src) => match self.cache.get(&OutPinId::from(*src)) {
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
        matches!(node, NodeKind::Constant(_) | NodeKind::Concat { .. })
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
        if let NodeKind::Concat { count } = &mut snarl[node] {
            ui.horizontal(|ui| {
                if ui.small_button("+").clicked() {
                    *count += 1;
                }
                if *count > 1 && ui.small_button("-").clicked() {
                    *count -= 1;
                }
                ui.label(format!("{} inputs", count));
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
            if ui.button("CONCAT").clicked() {
                snarl.insert_node(pos, NodeKind::Concat { count: 2 });
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

        if !self.functions.is_empty() {
            ui.separator();
            ui.menu_button("Call function", |ui| {
                for (i, f) in self.functions.iter().enumerate() {
                    let hash = f.graph_hash();
                    let name = self
                        .names
                        .functions
                        .get(&hash)
                        .filter(|n| !n.is_empty())
                        .cloned()
                        .unwrap_or_else(|| format!("#{}", &hash[..8]));
                    let (in_types, out_types) = f.call_types();
                    if ui.button(&name).clicked() {
                        snarl.insert_node(
                            pos,
                            NodeKind::FunctionCall {
                                def_index: i,
                                in_types,
                                out_types,
                            },
                        );
                        ui.close();
                    }
                }
            });
        }

        if self.fn_hash.is_none() {
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
                if ui.button("ADD").clicked()   { snarl.insert_node(pos, NodeKind::Add);   ui.close(); }
                if ui.button("SUB").clicked()   { snarl.insert_node(pos, NodeKind::Sub);   ui.close(); }
                if ui.button("AND").clicked()   { snarl.insert_node(pos, NodeKind::And);   ui.close(); }
                if ui.button("SLICE").clicked() { snarl.insert_node(pos, NodeKind::Slice); ui.close(); }
            }
            _ /* Byte */ => {
                if ui.button("AND").clicked()        { snarl.insert_node(pos, NodeKind::And);  ui.close(); }
                if ui.button("CONCAT (2)").clicked() { snarl.insert_node(pos, NodeKind::Concat { count: 2 }); ui.close(); }
                if ui.button("UNPACK").clicked()     { snarl.insert_node(pos, NodeKind::Unpack); ui.close(); }
                if self.fn_hash.is_none() && ui.button("SINK").clicked() { snarl.insert_node(pos, NodeKind::Sink); ui.close(); }
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

        if let NodeKind::Concat { count } = &mut snarl[node] {
            if ui.button("Add input").clicked() {
                *count += 1;
                ui.close();
            }
            if *count > 1 && ui.button("Remove last input").clicked() {
                *count -= 1;
                ui.close();
            }
            ui.separator();
        }

        if ui.button("Delete node").clicked() {
            snarl.remove_node(node);
            ui.close();
        }
    }
}
