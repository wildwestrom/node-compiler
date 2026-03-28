use crate::ui::logic::EvalCache;
use crate::ui::logic::NodeKind;
use crate::ui::logic::NodeValue;
use crate::ui::logic::WireType;
use egui_snarl::InPin;
use egui_snarl::OutPin;
use egui_snarl::OutPinId;
use egui_snarl::Snarl;
use egui_snarl::ui::AnyPins;
use egui_snarl::ui::SnarlPin;
use egui_snarl::ui::SnarlViewer;

pub(crate) struct NodeGraphViewer<'a> {
    pub(crate) cache: &'a EvalCache,
    pub(crate) fn_sigs: &'a [(String, Vec<WireType>, Vec<WireType>)],
    pub(crate) in_subgraph: bool,
}

impl NodeGraphViewer<'_> {
    /// Render the cached value for an output pin as dim monospace text.
    pub(crate) fn show_value(&self, pin: OutPinId, ui: &mut egui::Ui) {
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
            NodeKind::FunctionCall {
                def_index, name, ..
            } => self
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
