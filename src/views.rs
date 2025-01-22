#[allow(mutable_transmutes)]
use eframe::egui;
use rfd::MessageDialog;
use std::sync::atomic::{AtomicBool, Ordering};
#[derive(Default)]
pub enum ViewType {
    #[default]
    Home,
    Editor,
}
#[derive(Default)]
pub struct SearchState {
    open: bool,
    query: String,
    case_sensitive: bool,
    current_match: usize,
    matches: Vec<(usize, usize)>,
}
pub static WAS_MODIFIED: AtomicBool = AtomicBool::new(false);
static mut SEARCH_STATE: Option<SearchState> = None;
impl SearchState {
    fn find_matches(&mut self, text: &str) {
        self.matches.clear();
        if self.query.is_empty() {
            return;
        }

        let text_to_search = if self.case_sensitive {
            text.to_string()
        } else {
            text.to_lowercase()
        };
        let query = if self.case_sensitive {
            self.query.clone()
        } else {
            self.query.to_lowercase()
        };

        let mut start = 0;
        while let Some(found) = text_to_search[start..].find(&query) {
            let match_start = start + found;
            let match_end = match_start + query.len();
            self.matches.push((match_start, match_end));
            start = match_end;
        }
    }

    fn next_match(&mut self) {
        if !self.matches.is_empty() {
            self.current_match = (self.current_match + 1) % self.matches.len();
        }
    }

    fn prev_match(&mut self) {
        if !self.matches.is_empty() {
            self.current_match = if self.current_match == 0 {
                self.matches.len() - 1
            } else {
                self.current_match - 1
            };
        }
    }
}
pub fn show_top_panel(
    ctx: &egui::Context,
    filename: &mut String,
    text_content: &mut String,
    current_view: &mut ViewType,
) {
    let content_clone = text_content.clone(); // Clone it once
    let mut should_open_search = false;
    egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("Kokona", |ui| {
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
                            Err(e) => {
                                rfd::MessageDialog::new()
                                    .set_title("Error")
                                    .set_description(&format!("Error opening file: {}", e))
                                    .set_level(rfd::MessageLevel::Error)
                                    .show();
                            }
                        }
                    }
                    ui.close_menu();
                }
                if ui.button("Save").clicked() {
                    if filename == "untitled.txt" {
                        if let Some(path) = rfd::FileDialog::new()
                            .set_title("Save")
                            .set_file_name(&filename[..])
                            .save_file()
                        {
                            // Save the contents to the file
                            if let Err(e) = std::fs::write(&path, &text_content) {
                                println!("Error saving file: {}", e);
                            } else {
                                println!("File saved successfully to: {}", path.display());
                                WAS_MODIFIED.store(false, Ordering::SeqCst);
                            }
                            *filename = path.display().to_string();
                            ctx.send_viewport_cmd(egui::ViewportCommand::Title("Kokona".into()));
                        }
                    } else {
                        // Save directly to existing path
                        if let Err(e) = std::fs::write(&filename, &text_content) {
                            println!("Error saving file: {}", e);
                        } else {
                            println!("File saved successfully to: {}", filename);
                            WAS_MODIFIED.store(false, Ordering::SeqCst);
                        }

                        ctx.send_viewport_cmd(egui::ViewportCommand::Title("Kokona".into()));
                    }
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
                            WAS_MODIFIED.store(false, Ordering::SeqCst);
                        }
                        *filename = path.display().to_string();
                        ctx.send_viewport_cmd(egui::ViewportCommand::Title("Kokona".into()));
                    }
                    ui.close_menu();
                }
                if ui.button("Close").clicked() {
                    *filename = String::from("");
                    *current_view = ViewType::Home;
                    ui.close_menu();
                }
                if ui.button("Exit").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    ui.close_menu();
                }
            });
            ui.menu_button("File", |ui| {
                if ui.button("Search").clicked() {
                    should_open_search = true;
                    ui.close_menu();
                }
            });

            // After the menu bar, handle the search
            if should_open_search {
                unsafe {
                    if SEARCH_STATE.is_none() {
                        SEARCH_STATE = Some(SearchState::default());
                    }
                    if let Some(state) = SEARCH_STATE.as_mut() {
                        state.open = true;
                        state.find_matches(&content_clone);
                    }
                }
            }
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
                        ui.label(format!("Version {}", crate::consts::versioninfo::VERSION));
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
                ui.label(format!("ver {}", crate::consts::versioninfo::VERSION));
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
        *text = String::new();
        ctx.send_viewport_cmd(egui::ViewportCommand::Title("Kokona".into()));
    }

    show_top_panel(ctx, filename, &mut String::from(""), current_view);
}

pub fn editor_view(
    ctx: &egui::Context,
    text: &mut String,
    filename: &mut String,
    current_view: &mut ViewType,
) -> bool {
    let mut was_modified = WAS_MODIFIED.load(Ordering::SeqCst);
    //static mut SEARCH_STATE: Option<SearchState> = None;
    unsafe {
        if SEARCH_STATE.is_none() {
            SEARCH_STATE = Some(SearchState::default());
        }
    }

    show_top_panel(ctx, filename, text, current_view);

    // Check for Ctrl+F
    if ctx.input(|i| i.key_pressed(egui::Key::F) && i.modifiers.command) {
        unsafe {
            if let Some(state) = SEARCH_STATE.as_mut() {
                state.open = true;
                state.find_matches(text);
            }
        }
    }
    if ctx.input(|i| i.key_pressed(egui::Key::S) && i.modifiers.command) {
        if filename == "untitled.txt" {
            if let Some(path) = rfd::FileDialog::new()
                .set_title("Save")
                .set_file_name(&filename[..])
                .save_file()
            {
                // Save the contents to the file
                if let Err(e) = std::fs::write(&path, &text) {
                    println!("Error saving file: {}", e);
                } else {
                    println!("File saved successfully to: {}", path.display());
                    WAS_MODIFIED.store(false, Ordering::SeqCst);
                }
                *filename = path.display().to_string();
                ctx.send_viewport_cmd(egui::ViewportCommand::Title("Kokona".into()));
            }
        } else {
            // Save directly to existing path
            if let Err(e) = std::fs::write(&filename, &text) {
                println!("Error saving file: {}", e);
            } else {
                println!("File saved successfully to: {}", filename);
                WAS_MODIFIED.store(false, Ordering::SeqCst);
            }

            ctx.send_viewport_cmd(egui::ViewportCommand::Title("Kokona".into()));
        }
    }
    // Show search overlay if open
    unsafe {
        if let Some(state) = SEARCH_STATE.as_mut() {
            if state.open {
                egui::Window::new("Search")
                    .fixed_size([300.0, 100.0])
                    .collapsible(false)
                    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            let query_changed = ui.text_edit_singleline(&mut state.query).changed();
                            if query_changed {
                                state.find_matches(text);
                                state.current_match = 0;
                            }

                            if ui.button("×").clicked() {
                                state.open = false;
                            }
                        });

                        ui.horizontal(|ui| {
                            if ui
                                .checkbox(&mut state.case_sensitive, "Case sensitive")
                                .changed()
                            {
                                state.find_matches(text);
                            }

                            if ui.button("⬆ Previous").clicked()
                                || ui.input(|i| i.key_pressed(egui::Key::N) && i.modifiers.shift)
                            {
                                state.prev_match();
                            }
                            if ui.button("⬇ Next").clicked()
                                || ui.input(|i| i.key_pressed(egui::Key::N) && i.modifiers.command)
                            {
                                state.next_match();
                            }
                        });

                        ui.label(format!(
                            "{} matches found{}",
                            state.matches.len(),
                            if !state.matches.is_empty() {
                                format!(
                                    " (showing {}/{})",
                                    state.current_match + 1,
                                    state.matches.len()
                                )
                            } else {
                                String::new()
                            }
                        ));

                        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                            state.open = false;
                        }
                    });
            }
        }
    }

    egui::CentralPanel::default().show(ctx, |ui| {
        let available_width = ui.available_width();
        let available_height = ui.available_height() - 20.0;

        egui::ScrollArea::vertical()
            .max_height(available_height)
            .show(ui, |ui| {
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

                    // Text editor with highlighting
                    let text_edit = egui::TextEdit::multiline(text)
                        .desired_width(available_width - 50.0)
                        .min_size(egui::vec2(available_width - 50.0, available_height))
                        .font(egui::TextStyle::Monospace);

                    let response = unsafe {
                        if let Some(state) = SEARCH_STATE.as_mut() {
                            if state.open && !state.matches.is_empty() {
                                let mut layouter =
                                    |ui: &egui::Ui, string: &str, wrap_width: f32| {
                                        let mut layout_job = egui::text::LayoutJob::default();
                                        let mut last_end = 0;

                                        // Set default format to use monospace font
                                        let default_format = egui::TextFormat {
                                            font_id: egui::FontId::monospace(12.0), // Fixed font size
                                            ..Default::default()
                                        };

                                        for (idx, &(start, end)) in state.matches.iter().enumerate()
                                        {
                                            // Add non-highlighted text with monospace
                                            if last_end < start {
                                                layout_job.append(
                                                    &string[last_end..start],
                                                    0.0,
                                                    default_format.clone(),
                                                );
                                            }

                                            // Add highlighted text with monospace
                                            let format = if idx == state.current_match {
                                                egui::TextFormat {
                                                    background: egui::Color32::from_rgb(
                                                        255, 255, 0,
                                                    ),
                                                    font_id: egui::FontId::monospace(12.0),
                                                    ..Default::default()
                                                }
                                            } else {
                                                egui::TextFormat {
                                                    background: egui::Color32::from_rgb(
                                                        255, 255, 180,
                                                    ),
                                                    font_id: egui::FontId::monospace(12.0),
                                                    ..Default::default()
                                                }
                                            };

                                            layout_job.append(&string[start..end], 0.0, format);
                                            last_end = end;
                                        }

                                        // Add remaining text with monospace
                                        if last_end < string.len() {
                                            layout_job.append(
                                                &string[last_end..],
                                                0.0,
                                                default_format,
                                            );
                                        }

                                        ui.fonts(|f| f.layout_job(layout_job))
                                    };

                                let response = ui.add(text_edit.layouter(&mut layouter));

                                if response.changed() {
                                    WAS_MODIFIED.store(true, Ordering::SeqCst); // Set the flag
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Title(
                                        "Kokona | MODIFIED".into(),
                                    ));
                                }
                                response
                            } else {
                                let response = ui.add(text_edit);
                                if response.changed() {
                                    WAS_MODIFIED.store(true, Ordering::SeqCst); // Set the flag
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Title(
                                        "Kokona | MODIFIED".into(),
                                    ));
                                }
                                response
                            }
                        } else {
                            let response = ui.add(text_edit);
                            if response.changed() {
                                WAS_MODIFIED.store(true, Ordering::SeqCst); // Set the flag
                            }
                            response
                        }
                    };

                    let (line, col) = if response.has_focus() {
                        calculate_cursor_position(text)
                    } else {
                        (1, 1)
                    };

                    show_bottom_status_bar(ctx, line, col, text);
                });
            });
    });
    was_modified = WAS_MODIFIED.load(Ordering::SeqCst);
    was_modified
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
