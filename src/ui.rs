mod node_viewer;
mod persistence;
pub(crate) mod snarl_graph;

use crate::graph::{FunctionDef, NodeKind, WireType, eval_graph};
use crate::ui::snarl_graph::{auto_arrange, graph_from_snarl, snarl_from_graph};

use std::path::PathBuf;

use eframe::CreationContext;
use egui::Id;
use egui_snarl::{
    InPinId, OutPinId, Snarl,
    ui::{BackgroundPattern, SnarlStyle, SnarlWidget},
};
use log::{debug, warn};

// ─── App ─────────────────────────────────────────────────────────────────────

pub struct App {
    snarl: Snarl<NodeKind>,
    style: SnarlStyle,
    functions: Vec<FunctionDef>,
    /// `Some((idx, snarl))` = currently editing `functions[idx]` with this live snarl.
    editing: Option<(usize, Snarl<NodeKind>)>,
    /// Path to the `.ncg` file currently open.
    current_path: Option<PathBuf>,
    working_dir: PathBuf,
    error: Option<String>,
    /// Serialized bytes of the graph at the last save/load — used for dirty detection.
    last_saved_state: Vec<u8>,
    /// Names sidecar data.
    names: persistence::NamesData,
    /// Serialized bytes of names at the last save/load — used for dirty detection.
    last_saved_names: Vec<u8>,
    /// Pending action that triggered the "unsaved changes" dialog.
    pending_action: Option<PendingAction>,
}

#[derive(Clone, Copy)]
enum PendingAction {
    Close,
    New,
}

const STORAGE_KEY_LAST_PATH: &str = "last_path";

fn serialize_state(state: &persistence::SavedState) -> Vec<u8> {
    #[cfg(not(feature = "human-readable"))]
    return postcard::to_allocvec(state).unwrap_or_default();
    #[cfg(feature = "human-readable")]
    return ron::to_string(state).unwrap_or_default().into_bytes();
}

fn serialize_names(names: &persistence::NamesData) -> Vec<u8> {
    #[cfg(not(feature = "human-readable"))]
    return postcard::to_allocvec(names).unwrap_or_default();
    #[cfg(feature = "human-readable")]
    return ron::to_string(names).unwrap_or_default().into_bytes();
}

impl App {
    pub fn new(cx: &CreationContext) -> Self {
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

        let functions: Vec<FunctionDef> = Vec::new();
        let default_state = persistence::SavedState {
            root_graph: graph_from_snarl(&snarl),
            functions: functions.clone(),
        };
        let last_saved_state = serialize_state(&default_state);
        let default_names = persistence::NamesData::default();
        let last_saved_names = serialize_names(&default_names);

        let mut app = Self {
            snarl,
            style: SnarlStyle {
                bg_pattern: Some(BackgroundPattern::Grid(egui_snarl::ui::Grid {
                    spacing: (50.0, 50.0).into(),
                    angle: 0.0,
                })),
                pin_placement: Some(egui_snarl::ui::PinPlacement::Edge),
                ..Default::default()
            },
            functions,
            editing: None,
            current_path: None,
            working_dir: std::env::current_dir().unwrap_or_default(),
            error: None,
            last_saved_state,
            names: default_names,
            last_saved_names,
            pending_action: None,
        };

        // Reopen the last-used file if it still exists.
        if let Some(storage) = cx.storage {
            debug!("Storage exists");
            if let Some(path_str) = storage.get_string(STORAGE_KEY_LAST_PATH) {
                debug!("Got path_str: {path_str}");
                let path = PathBuf::from(path_str);
                if path.exists() {
                    match persistence::load_state(&path) {
                        Ok(state) => {
                            app.last_saved_state = serialize_state(&state);
                            app.snarl = snarl_from_graph(&state.root_graph);
                            app.functions = state.functions;
                            let names = persistence::load_names(&persistence::sidecar_path(&path));
                            app.last_saved_names = serialize_names(&names);
                            app.names = names;
                            app.current_path = Some(path);
                        }
                        Err(e) => {
                            app.error = Some(format!("Failed to reopen last file: {e}"));
                        }
                    }
                }
            } else {
                debug!("No save state path found: Will be created upon opening a file");
            }
        } else {
            warn!("No storage")
        }

        app
    }

    fn is_dirty(&self) -> bool {
        let current = persistence::SavedState {
            root_graph: graph_from_snarl(&self.snarl),
            functions: self.functions.clone(),
        };
        serialize_state(&current) != self.last_saved_state
            || serialize_names(&self.names) != self.last_saved_names
    }

    fn do_save(&mut self, path: &std::path::Path) -> anyhow::Result<()> {
        let state = persistence::SavedState {
            root_graph: graph_from_snarl(&self.snarl),
            functions: self.functions.clone(),
        };
        persistence::save_state(&state, path)?;
        self.last_saved_state = serialize_state(&state);
        let names_path = persistence::sidecar_path(path);
        persistence::save_names(&self.names, &names_path)?;
        self.last_saved_names = serialize_names(&self.names);
        Ok(())
    }

    /// Reset to a blank graph.
    fn do_new(&mut self) {
        self.snarl = Snarl::new();
        self.functions = Vec::new();
        self.editing = None;
        self.current_path = None;
        self.names = persistence::NamesData::default();
        let blank = persistence::SavedState {
            root_graph: graph_from_snarl(&self.snarl),
            functions: self.functions.clone(),
        };
        self.last_saved_state = serialize_state(&blank);
        self.last_saved_names = serialize_names(&self.names);
    }

    /// Save to the current path, or open a Save As dialog if none is set.
    fn handle_save(&mut self) -> anyhow::Result<()> {
        if let Some(path) = self.current_path.clone() {
            self.do_save(&path)
        } else if let Some(path) = rfd::FileDialog::new()
            .add_filter("Node graph", &["ncg"])
            .set_file_name("Untitled.ncg")
            .save_file()
        {
            self.do_save(&path)?;
            self.current_path = Some(path);
            Ok(())
        } else {
            Ok(())
        }
    }

    fn file_submenu(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.menu_button("File", |ui| {
            if ui.button("New").clicked() {
                ui.close();
                if self.is_dirty() {
                    self.pending_action = Some(PendingAction::New);
                } else {
                    self.do_new();
                }
            }
            if ui.button("Open…").clicked() {
                ui.close();
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Node graph", &["ncg"])
                    .set_directory(&self.working_dir)
                    .pick_file()
                {
                    match persistence::load_state(&path) {
                        Ok(state) => {
                            self.last_saved_state = serialize_state(&state);
                            self.snarl = snarl_from_graph(&state.root_graph);
                            self.functions = state.functions;
                            self.editing = None;
                            let names =
                                persistence::load_names(&persistence::sidecar_path(&path));
                            self.last_saved_names = serialize_names(&names);
                            self.names = names;
                            self.current_path = Some(path);
                        }
                        Err(e) => self.error = Some(format!("Failed to open: {e}")),
                    }
                }
            }
            if ui
                .add_enabled(self.current_path.is_some(), egui::Button::new("Save"))
                .clicked()
            {
                ui.close();
                if let Err(e) = self.handle_save() {
                    self.error = Some(format!("Failed to save: {e}"));
                }
            }
            if ui.button("Save As…").clicked() {
                ui.close();
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Node graph", &["ncg"])
                    .set_file_name("Untitled.ncg")
                    .save_file()
                {
                    match self.do_save(&path) {
                        Ok(()) => self.current_path = Some(path),
                        Err(e) => self.error = Some(format!("Failed to save: {e}")),
                    }
                }
            }
            ui.separator();
            if ui.button("Quit").clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
    }
}

impl eframe::App for App {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if let Some(path) = &self.current_path {
            storage.set_string(STORAGE_KEY_LAST_PATH, path.to_string_lossy().to_string());
        }
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Error dialog.
        if let Some(msg) = self.error.clone() {
            egui::Window::new("Error")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label(&msg);
                    ui.add_space(8.0);
                    if ui.button("OK").clicked() {
                        self.error = None;
                    }
                });
        }

        // Ctrl-S: save (or Save As if no path set).
        if ctx.input(|i| i.key_pressed(egui::Key::S) && i.modifiers.command)
            && let Err(e) = self.handle_save()
        {
            self.error = Some(format!("Failed to save: {e}"));
        }

        // Intercept close requests: if there are unsaved changes, show save/discard dialog.
        if ctx.input(|i| i.viewport().close_requested()) && self.is_dirty() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending_action = Some(PendingAction::Close);
        }

        // Snapshot function signatures to pass to the viewer without borrow conflicts.
        let fn_sigs: Vec<(String, Vec<WireType>, Vec<WireType>)> = self
            .functions
            .iter()
            .map(|f| {
                let (in_types, out_types) = f.call_types();
                let hash = f.graph_hash();
                let name = self
                    .names
                    .functions
                    .get(&hash)
                    .filter(|n| !n.is_empty())
                    .cloned()
                    .unwrap_or_else(|| format!("#{}", &hash[..8]));
                (name, in_types, out_types)
            })
            .collect();

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                self.file_submenu(ctx, ui);

                if ui.button("Arrange").clicked() {
                    match &mut self.editing {
                        None => auto_arrange(&mut self.snarl),
                        Some((_, editing_snarl)) => auto_arrange(editing_snarl),
                    }
                }

                if let Some(pathstr) = self
                    .current_path
                    .as_ref()
                    .and_then(|p| p.strip_prefix(&self.working_dir).unwrap_or(p).to_str())
                {
                    let dirty = self.is_dirty();
                    let label = if dirty {
                        format!("• {pathstr}")
                    } else {
                        pathstr.to_owned()
                    };
                    let center_x = ui.clip_rect().center().x;
                    let center_y = ui.cursor().min.y + ui.spacing().interact_size.y / 2.0;
                    ui.painter().text(
                        egui::pos2(center_x, center_y),
                        egui::Align2::CENTER_CENTER,
                        label,
                        egui::FontId::default(),
                        ui.visuals().text_color(),
                    );
                }
            });
        });

        // Breadcrumb bar when editing a function subgraph.
        // Extract idx without holding a borrow on self.editing across the panel.
        let editing_idx = self.editing.as_ref().map(|(i, _)| *i);
        let mut close_editing = false;

        if let Some(idx) = editing_idx {
            egui::TopBottomPanel::top("breadcrumb").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("< Root").clicked() {
                        close_editing = true;
                    }
                    ui.label("›");
                    let hash = self.functions[idx].graph_hash();
                    let name = self.names.functions.entry(hash).or_default();
                    ui.text_edit_singleline(name);
                });
            });
        }

        if close_editing && let Some((fi, editing_snarl)) = self.editing.take() {
            self.functions[fi].graph = graph_from_snarl(&editing_snarl);
        }

        // Save/discard dialog shown when the user has unsaved changes before Close or New.
        if let Some(action) = self.pending_action {
            egui::Window::new("Unsaved changes")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label("You have unsaved changes.");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() {
                            match self.handle_save() {
                                Ok(()) => {
                                    self.pending_action = None;
                                    match action {
                                        PendingAction::Close => {
                                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                        }
                                        PendingAction::New => self.do_new(),
                                    }
                                }
                                Err(e) => self.error = Some(format!("Failed to save: {e}")),
                            }
                        }
                        if ui.button("Discard").clicked() {
                            self.pending_action = None;
                            match action {
                                PendingAction::Close => {
                                    // Mark clean so the close-intercept doesn't re-trigger.
                                    let current = persistence::SavedState {
                                        root_graph: graph_from_snarl(&self.snarl),
                                        functions: self.functions.clone(),
                                    };
                                    self.last_saved_state = serialize_state(&current);
                                    self.last_saved_names = serialize_names(&self.names);
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                }
                                PendingAction::New => self.do_new(),
                            }
                        }
                        if ui.button("Cancel").clicked() {
                            self.pending_action = None;
                        }
                    });
                });
        }

        // Sidebar: function library.
        egui::SidePanel::left("function_sidebar").show(ctx, |ui| {
            ui.heading("Functions");
            if ui.button("+ New").clicked() {
                self.functions.push(FunctionDef::new());
                let i = self.functions.len() - 1;
                let editing_snarl = snarl_from_graph(&self.functions[i].graph);
                self.editing = Some((i, editing_snarl));
            }
            ui.separator();

            let n = self.functions.len();
            let mut to_delete: Option<usize> = None;
            let mut to_edit: Option<usize> = None;
            let mut to_add: Option<usize> = None;

            for i in 0..n {
                ui.horizontal(|ui| {
                    let hash = self.functions[i].graph_hash();
                    let display = self
                        .names
                        .functions
                        .get(&hash)
                        .filter(|n| !n.is_empty())
                        .map(String::as_str)
                        .unwrap_or("Unnamed");
                    ui.label(display);
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
                // Sync current editing session back before switching.
                if let Some((fi, ref current_snarl)) = self.editing {
                    let graph = graph_from_snarl(current_snarl);
                    self.functions[fi].graph = graph;
                }
                let editing_snarl = snarl_from_graph(&self.functions[i].graph);
                self.editing = Some((i, editing_snarl));
            }
            if let Some(i) = to_add {
                let (in_types, out_types) = self.functions[i].call_types();
                let node = NodeKind::FunctionCall {
                    def_index: i,
                    in_types,
                    out_types,
                };
                self.snarl.insert_node(egui::pos2(0.0, 0.0), node);
            }
            if let Some(i) = to_delete {
                // Remove every FunctionCall node that references the deleted function
                // and fix the def_index of nodes that reference later functions.
                // Must be done before self.functions.remove(i).

                // Root snarl.
                let to_remove: Vec<_> = self
                    .snarl
                    .node_ids()
                    .filter_map(|(id, n)| {
                        if let NodeKind::FunctionCall { def_index, .. } = n {
                            if *def_index == i {
                                return Some(id);
                            }
                        }
                        None
                    })
                    .collect();
                for id in to_remove {
                    self.snarl.remove_node(id);
                }
                let to_fix: Vec<_> = self
                    .snarl
                    .node_ids()
                    .filter_map(|(id, n)| {
                        if let NodeKind::FunctionCall { def_index, .. } = n {
                            if *def_index > i {
                                return Some(id);
                            }
                        }
                        None
                    })
                    .collect();
                for id in to_fix {
                    if let NodeKind::FunctionCall { def_index, .. } = &mut self.snarl[id] {
                        *def_index -= 1;
                    }
                }

                // Editing snarl (if open).
                let editing_is_deleted = matches!(&self.editing, Some((idx, _)) if *idx == i);
                if editing_is_deleted {
                    self.editing = None;
                } else if let Some((idx, editing_snarl)) = &mut self.editing {
                    let to_remove: Vec<_> = editing_snarl
                        .node_ids()
                        .filter_map(|(id, n)| {
                            if let NodeKind::FunctionCall { def_index, .. } = n {
                                if *def_index == i {
                                    return Some(id);
                                }
                            }
                            None
                        })
                        .collect();
                    for id in to_remove {
                        editing_snarl.remove_node(id);
                    }
                    let to_fix: Vec<_> = editing_snarl
                        .node_ids()
                        .filter_map(|(id, n)| {
                            if let NodeKind::FunctionCall { def_index, .. } = n {
                                if *def_index > i {
                                    return Some(id);
                                }
                            }
                            None
                        })
                        .collect();
                    for id in to_fix {
                        if let NodeKind::FunctionCall { def_index, .. } = &mut editing_snarl[id] {
                            *def_index -= 1;
                        }
                    }
                    if *idx > i {
                        *idx -= 1;
                    }
                }

                // Stored GraphData for every other function.
                for j in 0..self.functions.len() {
                    if j == i {
                        continue;
                    }
                    let graph = &mut self.functions[j].graph;
                    let deleted_ids: Vec<usize> = graph
                        .nodes
                        .iter()
                        .filter_map(|(id, n)| {
                            if let NodeKind::FunctionCall { def_index, .. } = n {
                                if *def_index == i {
                                    return Some(*id);
                                }
                            }
                            None
                        })
                        .collect();
                    graph.nodes.retain(|(_, n)| {
                        !matches!(n, NodeKind::FunctionCall { def_index, .. } if *def_index == i)
                    });
                    graph.wires.retain(|(out, inp)| {
                        !deleted_ids.contains(&out.node.0) && !deleted_ids.contains(&inp.node.0)
                    });
                    for (_, n) in &mut graph.nodes {
                        if let NodeKind::FunctionCall { def_index, .. } = n {
                            if *def_index > i {
                                *def_index -= 1;
                            }
                        }
                    }
                }

                self.functions.remove(i);
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| match &mut self.editing {
            None => {
                let cache = eval_graph(&self.snarl, &self.functions);
                SnarlWidget::new()
                    .id(Id::new("root_snarl"))
                    .style(self.style)
                    .show(
                        &mut self.snarl,
                        &mut node_viewer::NodeGraphViewer {
                            cache: &cache,
                            fn_sigs: &fn_sigs,
                            in_subgraph: false,
                        },
                        ui,
                    );
            }
            Some((idx, editing_snarl)) => {
                let idx = *idx;
                let cache = eval_graph(&*editing_snarl, &self.functions);
                SnarlWidget::new()
                    .id(Id::new(("fn_snarl", idx)))
                    .style(self.style)
                    .show(
                        editing_snarl,
                        &mut node_viewer::NodeGraphViewer {
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
