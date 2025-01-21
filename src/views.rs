use eframe::egui;
#[derive(Default)]
pub enum ViewType {
    #[default]
    Home,
    Editor,
}
pub fn show_top_panel(ctx: &egui::Context, filename: &mut String, text_content: &mut String) {
    egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Open").clicked() {
                    if let Some(path) = rfd::FileDialog::new().set_title("Open File").pick_file() {
                        // Read the file contents
                        match std::fs::read_to_string(&path) {
                            Ok(content) => {
                                *text_content = content; // Update the TextEdit content
                                *filename = path.display().to_string(); // Set filename to the path
                                ctx.send_viewport_cmd(egui::ViewportCommand::Title(
                                    "Kokona".into(),
                                ));
                                println!("File opened successfully from: {}", path.display());
                            }
                            Err(e) => println!("Error opening file: {}", e),
                        }
                    }
                    ui.close_menu();
                }
                if ui.button("Save").clicked() {
                    println!("Save clicked");
                    ui.close_menu();
                }
                if ui.button("Save As").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title("Save As")
                        .set_file_name(&filename[..])
                        .save_file()
                    {
                        // Save the contents to the file
                        if let Err(e) = std::fs::write(&path, text_content) {
                            println!("Error saving file: {}", e);
                        } else {
                            println!("File saved successfully to: {}", path.display());
                        }
                        *filename = path.display().to_string();
                        ctx.send_viewport_cmd(egui::ViewportCommand::Title("Kokona".into()));
                    }
                    ui.close_menu();
                }
                if ui.button("Exit").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    ui.close_menu();
                }
            });

            ui.menu_button("Edit", |ui| {
                if ui.button("Cut").clicked() {
                    println!("Cut clicked");
                    ui.close_menu();
                }
                if ui.button("Copy").clicked() {
                    println!("Copy clicked");
                    ui.close_menu();
                }
            });
            ui.menu_button("Help", |ui| {
                if ui.button("About").clicked() {
                    unsafe {
                        ABOUT_OPEN = true;
                    }
                    ui.close_menu();
                }
            });
            ui.horizontal(|ui| {
                ui.label("|");
                ui.label(&*filename);
            });
            static mut ABOUT_OPEN: bool = false;
            unsafe {
                egui::Window::new("About Kokona")
                    .open(&mut ABOUT_OPEN)
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                    .show(ctx, |ui| {
                        ui.heading("Kokona");
                        ui.label("Version 0.1");
                        ui.label("A simple text editor written in egui and Rust");
                        ui.add_space(8.0);
                        ui.label("Written by eri");
                    });
            }
        });
    });
}

pub fn home_view(
    ctx: &egui::Context,
    current_view: &mut ViewType,
    filename: &mut String,
    text: &mut String,
) {
    let mut should_create_new = false; // flag for new file

    egui::CentralPanel::default().show(ctx, |ui| {
        ui.add_space(15.0);
        ui.horizontal(|ui| {
            ui.heading(egui::RichText::new("Kokona").size(72.0));
            ui.vertical(|ui| {
                ui.add_space(55.5);
                ui.label("ver 0.1");
            });
        });
        ui.add_space(30.0);
        ui.group(|ui| {
            ui.vertical(|ui| {
                ui.label("Start");
                ui.add_space(5.0);
                if ui.button("New File").clicked() {
                    should_create_new = true;
                    *current_view = ViewType::Editor;
                }
                if ui.button("Open File").clicked() {
                    if let Some(path) = rfd::FileDialog::new().set_title("Open File").pick_file() {
                        match std::fs::read_to_string(&path) {
                            Ok(content) => {
                                *current_view = ViewType::Editor; // Switch to editor view
                                *filename = path.display().to_string(); // Set the filename
                                *text = content; // This will be shown in the TextEdit
                            }
                            Err(e) => println!("Error opening file: {}", e),
                        }
                    }
                }
            });
        });
        ui.with_layout(egui::Layout::bottom_up(egui::Align::RIGHT), |ui| {
            ui.label("kokona.nijika.dev");
        });
    });

    // Handle filename change after UI
    if should_create_new {
        *filename = String::from("untitled.txt");
    }

    show_top_panel(ctx, filename, &mut String::from(""));
}

pub fn editor_view(ctx: &egui::Context, text: &mut String, filename: &mut String) {
    show_top_panel(ctx, filename, text);
    egui::CentralPanel::default().show(ctx, |ui| {
        let available_width = ui.available_width();
        let available_height = ui.available_height() - 20.0; // Reserve space for bottom bar
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.horizontal(|ui| {
                // Line numbers
                let lines = text.split('\n').count().max(1);
                let line_numbers = (1..=lines)
                    .map(|i| i.to_string())
                    .collect::<Vec<_>>()
                    .join("\n");
                ui.add(
                    egui::TextEdit::multiline(&mut line_numbers.as_str())
                        .desired_width(35.0)
                        .min_size(egui::vec2(35.0, available_height))
                        .interactive(false)
                        .font(egui::TextStyle::Monospace)
                        .horizontal_align(egui::Align::RIGHT),
                );

                // Main text editor
                let response = ui.add(
                    egui::TextEdit::multiline(text)
                        .desired_width(available_width - 50.0)
                        .min_size(egui::vec2(available_width - 50.0, available_height))
                        .font(egui::TextStyle::Monospace),
                );
                if response.changed() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Title("Kokona | MODIFIED".into()));
                }
                // Get cursor position
                let (line, col) = if response.has_focus() {
                    calculate_cursor_position(text)
                } else {
                    (1, 1)
                };

                show_bottom_status_bar(ctx, line, col, text);
            });
        });
    });
}

fn calculate_cursor_position(text: &str) -> (usize, usize) {
    let mut line = 1;
    let mut col = 1;

    let char_idx = text.len();
    for (_i, c) in text[..char_idx].chars().enumerate() {
        if c == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }

    (line, col)
}
fn show_bottom_status_bar(ctx: &egui::Context, line: usize, col: usize, text: &str) {
    egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(5.0);
            ui.label(format!(
                "Line {}, Column {} | Characters: {}",
                line,
                col,
                text.len()
            ));
        });
    });
}
