#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};
use std::time::SystemTime;
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

struct MyApp {
    current_view: ViewType,
    opened: String,
    filename: String,
    show_confirm_dialog: bool,
    is_modified: bool,
    initial_file: Option<String>,
    discord: Option<DiscordIpcClient>,
    start_time: u64,
    fps: f32,
    last_time: std::time::Instant,
    fps_history: Vec<f32>,
}

impl MyApp {
    fn new(initial_file: Option<String>) -> Self {
        println!("Initializing Discord RPC...");
        let mut discord = match DiscordIpcClient::new("1332013100097863780") {
            Ok(client) => {
                println!("Discord client created successfully");
                client
            }
            Err(e) => {
                println!("Failed to create Discord client: {:?}", e);
                panic!("Discord client creation failed");
            }
        };

        match discord.connect() {
            Ok(_) => println!("Connected to Discord successfully"),
            Err(e) => println!("Failed to connect to Discord: {:?}", e),
        }

        let start_time = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            current_view: ViewType::default(),
            opened: String::new(),
            filename: String::new(),
            show_confirm_dialog: false,
            is_modified: false,
            initial_file,
            discord: Some(discord),
            start_time,
            fps: 0.0,
            last_time: std::time::Instant::now(),
            fps_history: Vec::with_capacity(100),
        }
    }
    fn ui(&mut self, ctx: &egui::Context) {
        let now = std::time::Instant::now();
        let frame_time = (now - self.last_time).as_secs_f32();
        self.last_time = now;

        self.fps_history.push(1.0 / frame_time);
        if self.fps_history.len() > 100 {
            self.fps_history.remove(0);
        }

        self.fps = self.fps_history.iter().sum::<f32>() / self.fps_history.len() as f32;

        egui::Window::new("FPS")
            .fixed_pos(egui::pos2(5.0, 5.0))
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(format!("FPS: {:.1}", self.fps));
            });
    }

    fn update_discord_presence(&mut self) {
        if let Some(discord) = &mut self.discord {
            println!("Updating Discord presence...");

            let details = match self.current_view {
                ViewType::Home => "On Home Screen",
                ViewType::Editor => {
                    if self.filename.is_empty() {
                        "Editing Untitled File"
                    } else {
                        "Editing File"
                    }
                }
            };

            let state = if !self.filename.is_empty() {
                format!(
                    "Working on: {}",
                    std::path::Path::new(&self.filename)
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                )
            } else {
                "No file open".to_string()
            };

            println!("Setting activity - Details: {}, State: {}", details, state);

            let kkv = crate::consts::versioninfo::VERSION;
            let version_text = format!("Kokona Text Editor version {}", kkv);
            let activity = activity::Activity::new()
                .state(&state)
                .details(details)
                .assets(
                    activity::Assets::new()
                        .large_image("kokona")
                        .large_text(&version_text),
                )
                .timestamps(activity::Timestamps::new().start(self.start_time.try_into().unwrap()));

            match discord.set_activity(activity) {
                Ok(_) => println!("Successfully set Discord activity"),
                Err(e) => {
                    println!("Failed to set Discord activity: {:?}", e);
                    println!("Attempting to reconnect...");
                    if let Err(e) = discord.connect() {
                        println!("Reconnection failed: {:?}", e);
                    }
                }
            }
        } else {
            println!("Discord client is None");
        }
    }
}
impl App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        self.update_discord_presence();
        //        self.ui(ctx); // uncommenting this shows you an FPS counter. this is debug functionality
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
        if ctx.input(|i| i.viewport().close_requested()) && !self.show_confirm_dialog {
            let modif = WAS_MODIFIED.load(Ordering::SeqCst);
            if modif {
                self.show_confirm_dialog = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                return;
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
        }

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

    let mut app = MyApp::new(cli.file);

    eframe::run_native("Kokona", options, Box::new(move |_cc| Ok(Box::new(app))))
}
