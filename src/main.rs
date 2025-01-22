#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub mod consts;
mod views;
use clap::Parser;
use eframe::{egui, App, Frame, NativeOptions};
use std::sync::atomic::Ordering;
use views::ViewType;
use views::WAS_MODIFIED;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    file: Option<String>,
}
#[derive(Default)]
struct MyApp {
    current_view: ViewType,
    opened: String,
    filename: String,
    show_confirm_dialog: bool,
    is_modified: bool,
    initial_file: Option<String>,
}

impl App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        if let Some(file_path) = self.initial_file.take() {
            match std::fs::read_to_string(&file_path) {
                Ok(content) => {
                    self.current_view = ViewType::Editor;
                    self.filename = file_path;
                    self.opened = content;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Title("Kokona".into()));
                }
                Err(e) => {
                    rfd::MessageDialog::new()
                        .set_title("Error")
                        .set_description(&format!("Error opening file: {}", e))
                        .set_level(rfd::MessageLevel::Error)
                        .show();
                }
            }
        }
        // Handle close request first, before any other updates
        if ctx.input(|i| i.viewport().close_requested()) && !self.show_confirm_dialog {
            let modif = WAS_MODIFIED.load(Ordering::SeqCst);
            if modif {
                self.show_confirm_dialog = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                return; // Exit early to prevent the close
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
        }

        // Show dialog if needed
        if self.show_confirm_dialog {
            egui::Window::new("Unsaved Changes")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .fixed_size([300.0, 65.0])
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label("You have unsaved changes.");
                        ui.label("Would you really like to close Kokona?");
                        ui.add_space(8.0);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Yes").clicked() {
                                WAS_MODIFIED.store(false, Ordering::SeqCst);
                                self.show_confirm_dialog = false;
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                return;
                            }
                            if ui.button("No").clicked() {
                                self.show_confirm_dialog = false;
                            }
                        });
                    });
                });
        }

        match self.current_view {
            ViewType::Home => views::home_view(
                ctx,
                &mut self.current_view,
                &mut self.filename,
                &mut self.opened,
            ),
            ViewType::Editor => {
                let modified = views::editor_view(
                    ctx,
                    &mut self.opened,
                    &mut self.filename,
                    &mut self.current_view,
                );
                if modified {
                    self.is_modified = true;
                }
            }
        }

        //        if ctx.input(|i| i.key_pressed(egui::Key::Tab)) {
        //            self.current_view = match self.current_view {
        //                ViewType::Home => ViewType::Editor,
        //                ViewType::Editor => ViewType::Home,
        //            };
        //        }
        // uncommenting this allows you to hit "Tab" to switch views
        // this is debug behavior that should not be included in release builds
        // except that it was in v0.1. oh well
    }
}
fn main() -> Result<(), eframe::Error> {
    let cli = Cli::parse();

    let options = NativeOptions {
        vsync: true,
        multisampling: 4,
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 720.0]),
        ..Default::default()
    };

    let mut app = MyApp::default();
    app.initial_file = cli.file;

    eframe::run_native("Kokona", options, Box::new(move |_cc| Ok(Box::new(app))))
}
