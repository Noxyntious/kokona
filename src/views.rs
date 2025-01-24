use eframe::egui;
use std::sync::atomic::{AtomicBool, Ordering};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;

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

pub struct EditorState {
    ps: SyntaxSet,
    syntax: Option<SyntaxReference>,
    theme: Theme,
    cached_highlights: Vec<(egui::TextFormat, String)>,
    last_text: String, // rhythm game waiter be like: this is your last dish
}

impl EditorState {
    pub fn new() -> Self {
        let ps = SyntaxSet::load_defaults_newlines();
        let ts = ThemeSet::load_defaults();
        let theme = ts.themes["base16-ocean.dark"].clone();

        Self {
            ps,
            syntax: None,
            theme,
            cached_highlights: Vec::new(),
            last_text: String::new(),
        }
    }

    pub fn set_syntax_for_extension(&mut self, filename: &str) {
        self.syntax = self
            .ps
            .find_syntax_for_file(filename)
            .ok()
            .flatten()
            .cloned();
    }

    pub fn get_or_update_highlights(&mut self, text: &str) -> &Vec<(egui::TextFormat, String)> {
        if self.last_text != text {
            self.last_text = text.to_string();

            if let Some(syntax) = &self.syntax {
                let mut h = HighlightLines::new(syntax, &self.theme);
                let mut highlights = Vec::new();

                for (i, line) in LinesWithEndings::from(text).enumerate() {
                    if i > 1000 {
                        let processed_len = highlights
                            .iter()
                            .map(|pair: &(egui::TextFormat, String)| pair.1.len())
                            .sum::<usize>();
                        highlights.push((
                            egui::TextFormat::default(),
                            text[processed_len..].to_string(),
                        ));
                        break;
                    }

                    if let Ok(line_highlights) = h.highlight_line(line, &self.ps) {
                        for (style, text) in line_highlights {
                            let format = egui::TextFormat {
                                color: egui::Color32::from_rgb(
                                    style.foreground.r,
                                    style.foreground.g,
                                    style.foreground.b,
                                ),
                                font_id: egui::FontId::monospace(12.0),
                                ..Default::default()
                            };
                            highlights.push((format, text.to_string()));
                        }
                    }
                }

                self.cached_highlights = highlights;
            } else {
                self.cached_highlights = vec![(
                    egui::TextFormat {
                        font_id: egui::FontId::monospace(12.0),
                        ..Default::default()
                    },
                    text.to_string(),
                )];
            }
        }

        &self.cached_highlights
    }
}
pub static WAS_MODIFIED: AtomicBool = AtomicBool::new(false);
static mut SEARCH_STATE: Option<SearchState> = None;
static mut EDITOR_STATE: Option<EditorState> = None;
static mut CACHED_LINE_NUMBERS: Option<(String, usize)> = None;

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
    let content_clone = text_content.clone();
    egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("Kokona", |ui| {
                if ui.button("Open").clicked() {
                    if let Some(path) = rfd::FileDialog::new().set_title("Open File").pick_file() {
                        // Read the file contents first
                        match std::fs::read_to_string(&path) {
                            Ok(content) => {
                                *text_content = content; // Update the TextEdit content
                                *filename = path.display().to_string(); // Set filename to the path
                                                                        // Switch view after content is loaded
                                *current_view = ViewType::Editor;
                                unsafe {
                                    if let Some(editor_state) = EDITOR_STATE.as_mut() {
                                        editor_state.set_syntax_for_extension(&filename);
                                    }
                                }
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
                    WAS_MODIFIED.store(false, Ordering::SeqCst);
                    ctx.send_viewport_cmd(egui::ViewportCommand::Title("Kokona".into()));
                    ui.close_menu();
                }
                if ui.button("Exit").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    ui.close_menu();
                }
            });
            ui.menu_button("File", |ui| {
                if ui.button("Search").clicked() {
                    unsafe {
                        if SEARCH_STATE.is_none() {
                            SEARCH_STATE = Some(SearchState::default());
                        }
                        if let Some(state) = SEARCH_STATE.as_mut() {
                            state.open = true;
                            state.find_matches(&content_clone);
                        }
                    }
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
                    unsafe {
                        if let Some(editor_state) = EDITOR_STATE.as_mut() {
                            editor_state.set_syntax_for_extension(&filename);
                        }
                    }
                }
                if ui.button("Open File").clicked() {
                    if let Some(path) = rfd::FileDialog::new().set_title("Open File").pick_file() {
                        match std::fs::read_to_string(&path) {
                            Ok(content) => {
                                *current_view = ViewType::Editor; // Switch to editor view
                                *filename = path.display().to_string(); // Set the filename
                                *text = content; // This will be shown in the TextEdit
                                unsafe {
                                    if let Some(editor_state) = EDITOR_STATE.as_mut() {
                                        editor_state.set_syntax_for_extension(&filename);
                                    }
                                }
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
    // Check for Ctrl+O
    if ctx.input(|i| i.key_pressed(egui::Key::O) && i.modifiers.command) {
        if let Some(path) = rfd::FileDialog::new().set_title("Open File").pick_file() {
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    *current_view = ViewType::Editor; // Switch to editor view
                    *filename = path.display().to_string(); // Set the filename
                    *text = content; // This will be shown in the TextEdit
                    unsafe {
                        if let Some(editor_state) = EDITOR_STATE.as_mut() {
                            editor_state.set_syntax_for_extension(&filename);
                        }
                    }
                }
                Err(e) => println!("Error opening file: {}", e),
            }
        }
    }
    show_top_panel(ctx, filename, text, current_view);
}

pub fn editor_view(
    ctx: &egui::Context,
    text: &mut String,
    filename: &mut String,
    current_view: &mut ViewType,
) -> bool {
    let mut was_modified = WAS_MODIFIED.load(Ordering::SeqCst);

    unsafe {
        if SEARCH_STATE.is_none() {
            SEARCH_STATE = Some(SearchState::default());
        }
        if EDITOR_STATE.is_none() {
            EDITOR_STATE = Some(EditorState::new());
            if let Some(state) = EDITOR_STATE.as_mut() {
                state.set_syntax_for_extension(filename);
            }
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

    // Check for Ctrl+O
    if ctx.input(|i| i.key_pressed(egui::Key::O) && i.modifiers.command) {
        if let Some(path) = rfd::FileDialog::new().set_title("Open File").pick_file() {
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    *current_view = ViewType::Editor; // Switch to editor view
                    *filename = path.display().to_string(); // Set the filename
                    *text = content; // This will be shown in the TextEdit
                    unsafe {
                        if let Some(editor_state) = EDITOR_STATE.as_mut() {
                            editor_state.set_syntax_for_extension(&filename);
                        }
                    }
                }
                Err(e) => println!("Error opening file: {}", e),
            }
        }
    }

    // Check for Ctrl+S
    if ctx.input(|i| i.key_pressed(egui::Key::S) && i.modifiers.command) {
        if filename == "untitled.txt" {
            if let Some(path) = rfd::FileDialog::new()
                .set_title("Save")
                .set_file_name(&filename[..])
                .save_file()
            {
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
            if let Err(e) = std::fs::write(&filename, &text) {
                println!("Error saving file: {}", e);
            } else {
                println!("File saved successfully to: {}", filename);
                WAS_MODIFIED.store(false, Ordering::SeqCst);
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Title("Kokona".into()));
        }
    }

    let text_ref = text.clone(); // Clone the text before the search window

    // Show search window if open
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
                                state.find_matches(&text_ref);
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
                                state.find_matches(&text_ref);
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
                    let line_numbers = unsafe {
                        let current_lines = text.split('\n').count().max(1);
                        let text_layout = ui.available_size_before_wrap();
                        let chars_per_line = (text_layout.x / 8.0).max(1.0) as usize;

                        if let Some((cached, len)) = &CACHED_LINE_NUMBERS {
                            if len == &current_lines {
                                cached.clone()
                            } else {
                                let new_numbers = (1..=current_lines)
                                    .flat_map(|i| {
                                        let line = text.lines().nth(i - 1).unwrap_or("");
                                        let wrap_count = if chars_per_line > 0 {
                                            (line.len() + chars_per_line - 1) / chars_per_line
                                        } else {
                                            1
                                        };
                                        std::iter::once(i.to_string()).chain(
                                            std::iter::repeat("-".to_string())
                                                .take(wrap_count.saturating_sub(1)),
                                        )
                                    })
                                    .collect::<Vec<_>>()
                                    .join("\n");

                                CACHED_LINE_NUMBERS = Some((new_numbers.clone(), current_lines));
                                new_numbers
                            }
                        } else {
                            let new_numbers = (1..=current_lines)
                                .flat_map(|i| {
                                    let line = text.lines().nth(i - 1).unwrap_or("");
                                    let wrap_count = if chars_per_line > 0 {
                                        (line.len() + chars_per_line - 1) / chars_per_line
                                    } else {
                                        1
                                    };
                                    std::iter::once(i.to_string()).chain(
                                        std::iter::repeat("-".to_string())
                                            .take(wrap_count.saturating_sub(1)),
                                    )
                                })
                                .collect::<Vec<_>>()
                                .join("\n");

                            CACHED_LINE_NUMBERS = Some((new_numbers.clone(), current_lines));
                            new_numbers
                        }
                    };

                    ui.add(
                        egui::TextEdit::multiline(&mut line_numbers.as_str())
                            .desired_width(35.0)
                            .min_size(egui::vec2(35.0, available_height))
                            .interactive(false)
                            .font(egui::TextStyle::Monospace)
                            .horizontal_align(egui::Align::RIGHT),
                    );

                    let text_edit = egui::TextEdit::multiline(text)
                        .desired_width(available_width - 50.0)
                        .min_size(egui::vec2(available_width - 50.0, available_height))
                        .font(egui::TextStyle::Monospace);

                    let response = unsafe {
                        let mut layouter = |ui: &egui::Ui, string: &str, wrap_width: f32| {
                            let mut layout_job = egui::text::LayoutJob::default();

                            if let Some(editor_state) = EDITOR_STATE.as_mut() {
                                let highlights = editor_state.get_or_update_highlights(string);
                                let mut last_end = 0;

                                let (search_highlights, current_match) =
                                    if let Some(search_state) = SEARCH_STATE.as_ref() {
                                        if search_state.open {
                                            (
                                                &search_state.matches[..],
                                                Some(search_state.current_match),
                                            )
                                        } else {
                                            (&[][..], None)
                                        }
                                    } else {
                                        (&[][..], None)
                                    };

                                for (format, text) in highlights {
                                    let text_start = last_end;
                                    let text_end = text_start + text.len();

                                    let format = format.clone();

                                    let mut processed = false;
                                    // Apply search highlighting if needed
                                    for (idx, &(s, e)) in search_highlights.iter().enumerate() {
                                        if s >= text_start && e <= text_end {
                                            let relative_start = s - text_start;
                                            let relative_end = e - text_start;

                                            // Add text before highlight
                                            if relative_start > 0 {
                                                layout_job.append(
                                                    &text[..relative_start],
                                                    0.0,
                                                    format.clone(),
                                                );
                                            }

                                            // Add highlighted text with different colors for current match
                                            let mut highlight_format = format.clone();
                                            highlight_format.background =
                                                if Some(idx) == current_match {
                                                    // Current match highlight - bright yellow
                                                    egui::Color32::from_rgb(255, 255, 0)
                                                } else {
                                                    // Other matches highlight - darker yellow
                                                    egui::Color32::from_rgb(180, 180, 0)
                                                };
                                            layout_job.append(
                                                &text[relative_start..relative_end],
                                                0.0,
                                                highlight_format,
                                            );

                                            // Add text after highlight
                                            if relative_end < text.len() {
                                                layout_job.append(
                                                    &text[relative_end..],
                                                    0.0,
                                                    format.clone(),
                                                );
                                            }

                                            processed = true;
                                            break;
                                        }
                                    }

                                    if !processed {
                                        layout_job.append(&text, 0.0, format);
                                    }

                                    last_end = text_end;
                                }
                            } else {
                                layout_job.append(string, 0.0, egui::TextFormat::default());
                            }
                            layout_job.wrap.max_width = wrap_width;
                            ui.fonts(|f| f.layout_job(layout_job))
                        };

                        let response = ui.add(text_edit.layouter(&mut layouter));
                        if response.changed() {
                            WAS_MODIFIED.store(true, Ordering::SeqCst);
                            ctx.send_viewport_cmd(egui::ViewportCommand::Title(
                                "Kokona | MODIFIED".into(),
                            ));
                        }
                        response
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
