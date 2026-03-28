mod logic;
use crate::ui::logic::{FunctionDef, NodeKind, WireType, eval_graph};

mod node_viewer;
mod persistence;

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
    // Some(idx) = currently editing functions[idx].graph
    editing: Option<usize>,
    /// Path to the `.ncg` file currently open.
    current_path: Option<PathBuf>,
    working_dir: PathBuf,
    error: Option<String>,
    /// Serialized bytes of the graph at the last save/load — used for dirty detection.
    last_saved_state: Vec<u8>,
    /// Set when a close request arrives while there are unsaved changes.
    pending_close: bool,
}

const STORAGE_KEY_LAST_PATH: &str = "last_path";

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
            snarl: snarl.clone(),
            functions: functions.clone(),
        };
        let last_saved_state = postcard::to_allocvec(&default_state).unwrap_or_default();

        let mut app = Self {
            snarl,
            style: SnarlStyle {
                bg_pattern: Some(BackgroundPattern::Grid(egui_snarl::ui::Grid {
                    spacing: (50.0, 50.0).into(),
                    angle: 0.0, //_f32.to_radians(),
                })),
                ..Default::default()
            },
            functions,
            editing: None,
            current_path: None,
            working_dir: std::env::current_dir().unwrap_or_default(),
            error: None,
            last_saved_state,
            pending_close: false,
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
                            app.last_saved_state =
                                postcard::to_allocvec(&state).unwrap_or_default();
                            app.snarl = state.snarl;
                            app.functions = state.functions;
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
            snarl: self.snarl.clone(),
            functions: self.functions.clone(),
        };
        postcard::to_allocvec(&current).unwrap_or_default() != self.last_saved_state
    }

    fn do_save(&mut self, path: &std::path::Path) -> anyhow::Result<()> {
        let state = persistence::SavedState {
            snarl: self.snarl.clone(),
            functions: self.functions.clone(),
        };
        persistence::save_state(&state, path)?;
        self.last_saved_state = postcard::to_allocvec(&state).unwrap_or_default();
        Ok(())
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

        // Intercept close requests: if there are unsaved changes, cancel the close and show the
        // save/discard dialog instead.
        // Ctrl-S: save (or Save As if no path set).
        if ctx.input(|i| i.key_pressed(egui::Key::S) && i.modifiers.command) {
            if let Err(e) = self.handle_save() {
                self.error = Some(format!("Failed to save: {e}"));
            }
        }

        if ctx.input(|i| i.viewport().close_requested()) && self.is_dirty() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending_close = true;
        }
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
                    if ui.button("Open…").clicked() {
                        ui.close();
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Node graph", &["ncg"])
                            .set_directory(&self.working_dir)
                            .pick_file()
                        {
                            match persistence::load_state(&path) {
                                Ok(state) => {
                                    self.last_saved_state =
                                        postcard::to_allocvec(&state).unwrap_or_default();
                                    self.snarl = state.snarl;
                                    self.functions = state.functions;
                                    self.editing = None;
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
                if let Some(pathstr) = self
                    .current_path
                    .as_ref()
                    .and_then(|p| p.strip_prefix(&self.working_dir).unwrap_or(p).to_str())
                {
                    let center_x = ui.clip_rect().center().x;
                    let center_y = ui.cursor().min.y + ui.spacing().interact_size.y / 2.0;
                    ui.painter().text(
                        egui::pos2(center_x, center_y),
                        egui::Align2::CENTER_CENTER,
                        pathstr,
                        egui::FontId::default(),
                        ui.visuals().text_color(),
                    );
                }
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

        // Save/discard dialog shown when the user closes with unsaved changes.
        if self.pending_close {
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
                                    self.pending_close = false;
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                }
                                Err(e) => self.error = Some(format!("Failed to save: {e}")),
                            }
                        }
                        if ui.button("Discard").clicked() {
                            self.pending_close = false;
                            // Sync last_saved_state so the close-request intercept
                            // doesn't fire again on the next frame.
                            let current = persistence::SavedState {
                                snarl: self.snarl.clone(),
                                functions: self.functions.clone(),
                            };
                            self.last_saved_state =
                                postcard::to_allocvec(&current).unwrap_or_default();
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        if ui.button("Cancel").clicked() {
                            self.pending_close = false;
                        }
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
                        &mut node_viewer::NodeGraphViewer {
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
