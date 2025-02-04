use directories_next::ProjectDirs;
use eframe::egui;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;

static UPDATE_CHECK_DONE: AtomicBool = AtomicBool::new(false);
static SHOULD_SHOW_UPDATE: OnceCell<(String, String)> = OnceCell::new();
static UPDATE_DIALOG_SHOWN: AtomicBool = AtomicBool::new(false);

static mut TERMINAL_PTY: Option<portable_pty::PtyPair> = None;
static mut TERMINAL_OUTPUT: Option<String> = None;
static mut TERMINAL_INPUT: Option<String> = None;
static mut TERMINAL_WRITER: Option<Box<dyn Write + Send>> = None;
static mut TERMINAL_OPEN: bool = false;

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct EditorSettings {
    pub font_size: f32, // Just one sample setting
}

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
    last_update: Instant,
    is_typing: bool,
}
fn compare_versions(current: &str, latest: &str) -> bool {
    println!(
        "Comparing versions - Current: {}, Latest: {}",
        current, latest
    );

    let current_parts: Vec<String> = current.split('.').map(|s| s.to_string()).collect();
    let latest_parts: Vec<String> = latest.split('.').map(|s| s.to_string()).collect();

    let current_parts: Vec<u32> = current_parts
        .iter()
        .enumerate()
        .map(|(i, s)| {
            if i == 2 {
                s.chars()
                    .next()
                    .unwrap_or('0')
                    .to_string()
                    .parse()
                    .unwrap_or(0)
            } else {
                s.parse().unwrap_or(0)
            }
        })
        .collect();

    let latest_parts: Vec<u32> = latest_parts
        .iter()
        .enumerate()
        .map(|(i, s)| {
            if i == 2 {
                // for the third number, only take the first character
                // if we dont do this it will break the updater if youre running a dev build like 0.3.3-dev
                s.chars()
                    .next()
                    .unwrap_or('0')
                    .to_string()
                    .parse()
                    .unwrap_or(0)
            } else {
                s.parse().unwrap_or(0)
            }
        })
        .collect();

    println!(
        "Parsed versions - Current: {:?}, Latest: {:?}",
        current_parts, latest_parts
    );

    for i in 0..3 {
        let current_num = current_parts.get(i).unwrap_or(&0);
        let latest_num = latest_parts.get(i).unwrap_or(&0);

        println!(
            "Comparing position {} - Current: {}, Latest: {}",
            i, current_num, latest_num
        );

        if latest_num > current_num {
            println!("Update needed!");
            return true;
        } else if latest_num < current_num {
            println!("Current version is newer!");
            return false;
        }
    }
    println!("Versions are equal");
    false
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
            last_update: Instant::now(),
            is_typing: false,
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
            self.is_typing = true;
            self.last_update = Instant::now();

            let line_count = text.matches('\n').count() + 1;
            //println!("Line count: {}", line_count);

            if line_count > 500 {
                //println!("Large file detected, using plain text");
                // if file exceeds 500 lines, disable real-time highlighting for performance
                self.cached_highlights = vec![(
                    egui::TextFormat {
                        font_id: egui::FontId::monospace(unsafe {
                            SETTINGS.as_ref().map_or(12.0, |s| s.font_size)
                        }),
                        ..Default::default()
                    },
                    text.to_string(),
                )];
                return &self.cached_highlights;
            }

            // Only reach this code for small files
            if let Some(syntax) = &self.syntax {
                let mut h = HighlightLines::new(syntax, &self.theme);
                let mut highlights = Vec::new();

                for line in LinesWithEndings::from(text) {
                    if let Ok(line_highlights) = h.highlight_line(line, &self.ps) {
                        for (style, text) in line_highlights {
                            let format = egui::TextFormat {
                                color: egui::Color32::from_rgb(
                                    style.foreground.r,
                                    style.foreground.g,
                                    style.foreground.b,
                                ),
                                font_id: egui::FontId::monospace(unsafe {
                                    SETTINGS.as_ref().map_or(12.0, |s| s.font_size)
                                }),
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
                        font_id: egui::FontId::monospace(unsafe {
                            SETTINGS.as_ref().map_or(12.0, |s| s.font_size)
                        }),
                        ..Default::default()
                    },
                    text.to_string(),
                )];
            }
        }

        // Check if we should update syntax highlighting for large files
        if self.is_typing && self.last_update.elapsed() >= Duration::from_millis(500) {
            let line_count = self.last_text.matches('\n').count() + 1;
            if line_count > 500 {
                self.is_typing = false;
                let current_text = self.last_text.clone();
                self.update_highlights(&current_text);
            }
        }

        &self.cached_highlights
    }

    pub fn force_highlight_update(&mut self) {
        self.is_typing = false;
        let current_text = self.last_text.clone();
        if let Some(syntax) = &self.syntax {
            let mut h = HighlightLines::new(syntax, &self.theme);
            let mut highlights = Vec::new();

            for line in LinesWithEndings::from(&current_text) {
                if let Ok(line_highlights) = h.highlight_line(line, &self.ps) {
                    for (style, text) in line_highlights {
                        let format = egui::TextFormat {
                            color: egui::Color32::from_rgb(
                                style.foreground.r,
                                style.foreground.g,
                                style.foreground.b,
                            ),
                            font_id: egui::FontId::monospace(unsafe {
                                SETTINGS.as_ref().map_or(12.0, |s| s.font_size)
                            }),
                            ..Default::default()
                        };
                        highlights.push((format, text.to_string()));
                    }
                }
            }

            self.cached_highlights = highlights;
        }
    }

    fn update_highlights(&mut self, text: &str) {
        if let Some(syntax) = &self.syntax {
            let mut h = HighlightLines::new(syntax, &self.theme);
            let mut highlights = Vec::new();

            for line in LinesWithEndings::from(text) {
                if let Ok(line_highlights) = h.highlight_line(line, &self.ps) {
                    for (style, text) in line_highlights {
                        let format = egui::TextFormat {
                            color: egui::Color32::from_rgb(
                                style.foreground.r,
                                style.foreground.g,
                                style.foreground.b,
                            ),
                            font_id: egui::FontId::monospace(unsafe {
                                SETTINGS.as_ref().map_or(12.0, |s| s.font_size)
                            }),
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
                    font_id: egui::FontId::monospace(unsafe {
                        SETTINGS.as_ref().map_or(12.0, |s| s.font_size)
                    }),
                    ..Default::default()
                },
                text.to_string(),
            )];
        }
    }
}
pub static WAS_MODIFIED: AtomicBool = AtomicBool::new(false);
static mut SEARCH_STATE: Option<SearchState> = None;
static mut EDITOR_STATE: Option<EditorState> = None;
static mut CACHED_LINE_NUMBERS: Option<(String, usize)> = None;
static mut SETTINGS: Option<EditorSettings> = None;
static mut SETTINGS_WINDOW_OPEN: bool = false;

impl Default for EditorSettings {
    fn default() -> Self {
        Self { font_size: 12.0 }
    }
}
impl EditorSettings {
    pub fn load() -> Self {
        if let Some(proj_dirs) = ProjectDirs::from("dev", "nijika", "kokona") {
            let config_dir = proj_dirs.config_dir();
            let config_file = config_dir.join("settings.json");

            if config_file.exists() {
                if let Ok(contents) = fs::read_to_string(config_file) {
                    if let Ok(settings) = serde_json::from_str(&contents) {
                        return settings;
                    }
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(proj_dirs) = ProjectDirs::from("dev", "nijika", "kokona") {
            let config_dir = proj_dirs.config_dir();
            fs::create_dir_all(config_dir)?;

            let config_file = config_dir.join("settings.json");
            let contents = serde_json::to_string_pretty(self)?;
            fs::write(config_file, contents)?;
        }
        Ok(())
    }
}
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
static mut GIT_RESULT_OPEN: bool = false;
static mut GIT_RESULT: Option<Result<std::process::Output, std::io::Error>> = None;
static mut INSERT_OPEN: bool = false;

static mut COMMIT_WINDOW_OPEN: bool = false;
static mut COMMIT_MESSAGE: String = String::new();
static mut COMMIT_RESULT: Option<Result<std::process::Output, std::io::Error>> = None;

static mut UNICHAR: String = String::new();

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
                            unsafe {
                                if let Some(editor_state) = EDITOR_STATE.as_mut() {
                                    editor_state.force_highlight_update();
                                }
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
                        unsafe {
                            if let Some(editor_state) = EDITOR_STATE.as_mut() {
                                editor_state.force_highlight_update();
                            }
                        }
                        *filename = path.display().to_string();
                        ctx.send_viewport_cmd(egui::ViewportCommand::Title("Kokona".into()));
                    }
                    ui.close_menu();
                }
                if ui.button("Settings").clicked() {
                    unsafe {
                        SETTINGS_WINDOW_OPEN = true;
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
                if ui.button("New file in working directory").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title("New file in working directory")
                        .save_file()
                    {
                        let newpath = path.display().to_string();
                        if let Err(e) = std::fs::write(&newpath, "") {
                            println!("Error creating file: {}", e);
                        }
                    }
                    ui.close_menu();
                }
                if ui.button("Insert character").clicked() {
                    unsafe {
                        INSERT_OPEN = true;
                    }
                    ui.close_menu();
                }
                if ui.button("Toggle terminal").clicked() {
                    unsafe { TERMINAL_OPEN = !TERMINAL_OPEN };
                    ui.close_menu();
                }
                if filename.ends_with(".rs") {
                    ui.menu_button("Rust options", |ui| {
                        if ui.button("Build").clicked() {
                            unsafe {
                                GIT_RESULT_OPEN = true;
                                let file_path = std::path::Path::new(&filename);
                                let parent_dir =
                                    file_path.parent().unwrap_or(std::path::Path::new(""));

                                GIT_RESULT = Some(
                                    std::process::Command::new("cargo")
                                        .current_dir(parent_dir)
                                        .arg("build")
                                        .output(),
                                );
                            }
                            ui.close_menu();
                        }
                        if ui.button("Run").clicked() {
                            unsafe {
                                TERMINAL_OPEN = true;
                                if TERMINAL_PTY.is_none() {
                                    if let Ok(pty_pair) = create_pty() {
                                        TERMINAL_PTY = Some(pty_pair);
                                        TERMINAL_OUTPUT = Some(String::new());
                                        TERMINAL_INPUT = Some(String::new());

                                        if let Some(pty_pair) = &TERMINAL_PTY {
                                            if let Ok(writer) = pty_pair.master.take_writer() {
                                                TERMINAL_WRITER = Some(writer);
                                            }

                                            let mut cmd =
                                                portable_pty::CommandBuilder::new("cargo");
                                            cmd.env("TERM", "dumb");
                                            cmd.arg("run");

                                            if let Some(parent) =
                                                std::path::Path::new(&filename).parent()
                                            {
                                                cmd.cwd(parent);
                                            }

                                            if let Some(writer) = &mut TERMINAL_WRITER {
                                                if let Ok(child) = pty_pair.slave.spawn_command(cmd)
                                                {
                                                    let mut reader =
                                                        pty_pair.master.try_clone_reader().unwrap();
                                                    std::thread::spawn(move || {
                                                        let mut buffer = [0u8; 1024];
                                                        loop {
                                                            match reader.read(&mut buffer) {
                                                                Ok(0) => break,
                                                                Ok(n) => {
                                                                    let str =
                                                                        String::from_utf8_lossy(
                                                                            &buffer[..n],
                                                                        )
                                                                        .into_owned();
                                                                    unsafe {
                                                                        if let Some(output) =
                                                                            &mut TERMINAL_OUTPUT
                                                                        {
                                                                            output.push_str(&str);
                                                                        }
                                                                    }
                                                                }
                                                                Err(_) => break,
                                                            }
                                                        }
                                                    });
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            ui.close_menu();
                        }
                        if ui.button("Check").clicked() {
                            unsafe {
                                GIT_RESULT_OPEN = true;
                                let file_path = std::path::Path::new(&filename);
                                let parent_dir =
                                    file_path.parent().unwrap_or(std::path::Path::new(""));

                                GIT_RESULT = Some(
                                    std::process::Command::new("cargo")
                                        .current_dir(parent_dir)
                                        .arg("check")
                                        .output(),
                                );
                            }
                            ui.close_menu();
                        }
                    });
                }
                if filename.ends_with(".py") {
                    ui.menu_button("Python options", |ui| {
                        if ui.button("Run").clicked() {
                            unsafe {
                                TERMINAL_OPEN = true;
                                if TERMINAL_PTY.is_none() {
                                    if let Ok(pty_pair) = create_pty() {
                                        TERMINAL_PTY = Some(pty_pair);
                                        TERMINAL_OUTPUT = Some(String::new());
                                        TERMINAL_INPUT = Some(String::new());

                                        if let Some(pty_pair) = &TERMINAL_PTY {
                                            if let Ok(writer) = pty_pair.master.take_writer() {
                                                TERMINAL_WRITER = Some(writer);
                                            }

                                            let mut cmd =
                                                portable_pty::CommandBuilder::new("python");
                                            cmd.env("TERM", "dumb");
                                            cmd.arg(&filename);

                                            if let Some(parent) =
                                                std::path::Path::new(filename).parent()
                                            {
                                                cmd.cwd(parent);
                                            }

                                            if let Some(writer) = &mut TERMINAL_WRITER {
                                                if let Ok(child) = pty_pair.slave.spawn_command(cmd)
                                                {
                                                    let mut reader =
                                                        pty_pair.master.try_clone_reader().unwrap();
                                                    std::thread::spawn(move || {
                                                        let mut buffer = [0u8; 1024];
                                                        loop {
                                                            match reader.read(&mut buffer) {
                                                                Ok(0) => break,
                                                                Ok(n) => {
                                                                    let str =
                                                                        String::from_utf8_lossy(
                                                                            &buffer[..n],
                                                                        )
                                                                        .into_owned();
                                                                    unsafe {
                                                                        if let Some(output) =
                                                                            &mut TERMINAL_OUTPUT
                                                                        {
                                                                            output.push_str(&str);
                                                                        }
                                                                    }
                                                                }
                                                                Err(_) => break,
                                                            }
                                                        }
                                                    });
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            ui.close_menu();
                        }
                    });
                }
            });

            ui.menu_button("Git", |ui| {
                if ui.button("Add current file").clicked() {
                    unsafe {
                        GIT_RESULT_OPEN = true;
                        // Get the parent directory of the file
                        let file_path = std::path::Path::new(&filename);
                        let parent_dir = file_path.parent().unwrap_or(std::path::Path::new(""));

                        // Create the command with the correct working directory
                        GIT_RESULT = Some(
                            std::process::Command::new("git")
                                .current_dir(parent_dir) // Set working directory to file's location
                                .arg("add")
                                .arg(file_path.file_name().unwrap_or_default()) // Use just the filename
                                .output(),
                        );
                    }
                }
                if ui.button("Commit").clicked() {
                    unsafe {
                        COMMIT_WINDOW_OPEN = true;
                        COMMIT_MESSAGE.clear(); // Clear any previous message
                    }
                    ui.close_menu();
                }
                ui.menu_button("More Git Options", |ui| {
                    if ui.button("Init").clicked() {
                        unsafe {
                            GIT_RESULT_OPEN = true;
                            let file_path = std::path::Path::new(&filename);
                            let parent_dir = file_path.parent().unwrap_or(std::path::Path::new(""));

                            GIT_RESULT = Some(
                                std::process::Command::new("git")
                                    .current_dir(parent_dir)
                                    .arg("init")
                                    .output(),
                            );
                        }
                        ui.close_menu();
                    }
                    if ui.button("Pull").clicked() {
                        unsafe {
                            GIT_RESULT_OPEN = true;
                            let file_path = std::path::Path::new(&filename);
                            let parent_dir = file_path.parent().unwrap_or(std::path::Path::new(""));

                            GIT_RESULT = Some(
                                std::process::Command::new("git")
                                    .current_dir(parent_dir)
                                    .arg("pull")
                                    .output(),
                            );
                        }
                        ui.close_menu();
                    }
                    if ui.button("Push").clicked() {
                        unsafe {
                            GIT_RESULT_OPEN = true;
                            let file_path = std::path::Path::new(&filename);
                            let parent_dir = file_path.parent().unwrap_or(std::path::Path::new(""));

                            GIT_RESULT = Some(
                                std::process::Command::new("git")
                                    .current_dir(parent_dir)
                                    .arg("push")
                                    .output(),
                            );
                        }
                        ui.close_menu();
                    }
                });
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

            unsafe {
                egui::Window::new("Insert character")
                    .open(&mut INSERT_OPEN)
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                    .show(ctx, |ui| {
                        ui.label("Type Unicode");
                        ui.horizontal(|ui| {
                            ui.label("U+");
                            static mut UNICODE_INPUT: String = String::new();
                            if ui
                                .text_edit_singleline(unsafe { &mut UNICODE_INPUT })
                                .changed()
                            {
                                if let Ok(code) = u32::from_str_radix(unsafe { &UNICODE_INPUT }, 16)
                                {
                                    if let Some(c) = char::from_u32(code) {
                                        UNICHAR = c.to_string();
                                    }
                                }
                            }

                            ui.add_space(10.0);

                            let mut display_text = UNICHAR.clone();
                            ui.add(
                                egui::TextEdit::singleline(&mut display_text)
                                    .interactive(true)
                                    .font(egui::TextStyle::Monospace)
                                    .background_color(egui::Color32::from_rgb(0, 0, 0)),
                            );
                        });
                    });
            }
            unsafe {
                if SETTINGS_WINDOW_OPEN {
                    egui::Window::new("Settings")
                        .open(&mut SETTINGS_WINDOW_OPEN)
                        .resizable(false)
                        .show(ctx, |ui| {
                            if let Some(settings) = &mut SETTINGS {
                                ui.vertical(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.label("Font Size:");
                                        let changed = ui
                                            .add(
                                                egui::DragValue::new(&mut settings.font_size)
                                                    .speed(0.5)
                                                    .clamp_range(8.0..=32.0),
                                            )
                                            .changed();

                                        if changed {
                                            settings.save().unwrap_or_else(|e| {
                                                println!("Failed to save settings: {}", e);
                                            });
                                            // Force highlights refresh
                                            if let Some(editor_state) = EDITOR_STATE.as_mut() {
                                                editor_state.cached_highlights.clear();
                                                editor_state.last_text.clear();
                                            }
                                            ctx.request_repaint();
                                        }
                                    });

                                    ui.separator();

                                    if ui.button("Reset to Defaults").clicked() {
                                        *settings = EditorSettings::default();
                                        settings.save().unwrap_or_else(|e| {
                                            println!("Failed to save settings: {}", e);
                                        });
                                        // Force highlights refresh
                                        if let Some(editor_state) = EDITOR_STATE.as_mut() {
                                            editor_state.cached_highlights.clear();
                                            editor_state.last_text.clear();
                                        }
                                        ctx.request_repaint();
                                    }
                                });
                            }
                        });
                }
            }
            unsafe {
                if GIT_RESULT_OPEN {
                    if let Some(result) = &GIT_RESULT {
                        match result {
                            Ok(output) => {
                                let stdout = String::from_utf8_lossy(&output.stdout);
                                let stderr = String::from_utf8_lossy(&output.stderr);

                                egui::Window::new("Git Result")
                                    .open(&mut GIT_RESULT_OPEN)
                                    .collapsible(false)
                                    .resizable(false)
                                    .fixed_size([300.0, 60.0])
                                    .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                                    .show(ctx, |ui| {
                                        if !stdout.is_empty() {
                                            ui.label(format!("Output: {}", stdout));
                                        }
                                        if !stderr.is_empty() {
                                            ui.label(format!("Error: {}", stderr));
                                        }
                                        if stdout.is_empty() && stderr.is_empty() {
                                            ui.label("File added successfully.");
                                        }
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::RIGHT),
                                            |ui| {
                                                if ui.button("Close").clicked() {
                                                    GIT_RESULT_OPEN = false;
                                                }
                                            },
                                        );
                                    });
                            }
                            Err(e) => {
                                egui::Window::new("Git Error")
                                    .open(&mut GIT_RESULT_OPEN)
                                    .collapsible(false)
                                    .resizable(false)
                                    .fixed_size([300.0, 60.0])
                                    .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                                    .show(ctx, |ui| {
                                        ui.label(format!("Error: {}", e));
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::RIGHT),
                                            |ui| {
                                                if ui.button("Close").clicked() {
                                                    GIT_RESULT_OPEN = false;
                                                }
                                            },
                                        );
                                    });
                            }
                        }
                    }
                }
            }
            unsafe {
                if COMMIT_WINDOW_OPEN {
                    egui::Window::new("Git Commit")
                        .open(&mut COMMIT_WINDOW_OPEN)
                        .collapsible(false)
                        .resizable(false)
                        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                        .show(ctx, |ui| {
                            ui.label("Enter commit message:");
                            ui.text_edit_singleline(&mut COMMIT_MESSAGE);
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.button("Commit").clicked() {
                                            let file_path = std::path::Path::new(&filename);
                                            let parent_dir = file_path
                                                .parent()
                                                .unwrap_or(std::path::Path::new(""));

                                            COMMIT_RESULT = Some(
                                                std::process::Command::new("git")
                                                    .current_dir(parent_dir)
                                                    .arg("commit")
                                                    .arg("-m")
                                                    .arg(&COMMIT_MESSAGE)
                                                    .output(),
                                            );
                                            COMMIT_WINDOW_OPEN = false;
                                        }
                                        if ui.button("Cancel").clicked() {
                                            COMMIT_WINDOW_OPEN = false;
                                        }
                                    },
                                );
                            });
                        });
                }

                // Show commit result if available
                if let Some(result) = &COMMIT_RESULT {
                    match result {
                        Ok(output) => {
                            let stdout = String::from_utf8_lossy(&output.stdout);
                            let stderr = String::from_utf8_lossy(&output.stderr);

                            egui::Window::new("Git Commit Result")
                                .collapsible(false)
                                .resizable(false)
                                .fixed_size([350.0, 60.0])
                                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                                .show(ctx, |ui| {
                                    if !stdout.is_empty() {
                                        ui.label(format!("Output: {}", stdout));
                                    }
                                    if !stderr.is_empty() {
                                        ui.label(format!("Error: {}", stderr));
                                    }
                                    if stdout.is_empty() && stderr.is_empty() {
                                        ui.label("Commit successful.");
                                    }
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::RIGHT),
                                        |ui| {
                                            if ui.button("Close").clicked() {
                                                COMMIT_RESULT = None;
                                            }
                                        },
                                    );
                                });
                        }
                        Err(e) => {
                            egui::Window::new("Git Commit Error")
                                .collapsible(false)
                                .resizable(false)
                                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                                .show(ctx, |ui| {
                                    ui.label(format!("Error: {}", e));
                                    if ui.button("Close").clicked() {
                                        COMMIT_RESULT = None;
                                    }
                                });
                        }
                    }
                }
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
    unsafe {
        SETTINGS = Some(EditorSettings::load());
    }
    let mut should_create_new = false; // flag for new file
    if let Some((current_version, latest_version)) = SHOULD_SHOW_UPDATE.get() {
        if !UPDATE_DIALOG_SHOWN.load(Ordering::SeqCst) {
            egui::Window::new("Update Available")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(format!(
                            "A new version of Kokona is available!\nCurrent: v{}\nLatest: v{}",
                            current_version, latest_version
                        ));
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.button("Open Download Page").clicked() {
                                        if let Err(e) = open::that(
                                            "https://github.com/Noxyntious/kokona/releases/latest",
                                        ) {
                                            println!("Failed to open URL: {}", e);
                                        }
                                        UPDATE_DIALOG_SHOWN.store(true, Ordering::SeqCst);
                                    }
                                    if ui.button("Dismiss").clicked() {
                                        UPDATE_DIALOG_SHOWN.store(true, Ordering::SeqCst);
                                    }
                                },
                            );
                        });
                    });
                });
        }
    }

    if !UPDATE_CHECK_DONE.load(Ordering::SeqCst) {
        UPDATE_CHECK_DONE.store(true, Ordering::SeqCst);

        let ctx_clone = ctx.clone();
        std::thread::spawn(move || {
            // wait for 2 seconds to let the application load
            std::thread::sleep(std::time::Duration::from_millis(500));

            std::thread::Builder::new()
                .name("update-checker".to_string())
                .spawn(move || {
                    let runtime = tokio::runtime::Runtime::new().unwrap();
                    runtime.block_on(async {
                        let client = reqwest::Client::new();
                        match client
                            .get("https://api.github.com/repos/Noxyntious/kokona/releases/latest")
                            .header("User-Agent", "kokona-update-checker")
                            .send()
                            .await
                        {
                            Ok(response) => {
                                match response.text().await {
                                    Ok(text) => {
                                        match serde_json::from_str::<GithubRelease>(&text) {
                                            Ok(release) => {
                                                let latest_version = release
                                                    .tag_name
                                                    .trim_start_matches('v')
                                                    .to_string();
                                                let current_version =
                                                    crate::consts::versioninfo::VERSION.to_string();

                                                if compare_versions(
                                                    &current_version,
                                                    &latest_version,
                                                ) {
                                                    let _ = SHOULD_SHOW_UPDATE
                                                        .set((current_version, latest_version));
                                                    ctx_clone.request_repaint();
                                                    // Request UI update
                                                }
                                            }
                                            Err(e) => println!("Failed to parse JSON: {}", e),
                                        }
                                    }
                                    Err(e) => println!("Failed to get response text: {}", e),
                                }
                            }
                            Err(e) => println!("Failed to get response from GitHub: {}", e),
                        }
                    });
                })
                .expect("Failed to spawn update checker thread");
        });
    }
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
            if ui.link("kokona.nijika.dev").clicked() {
                if let Err(e) = open::that("https://kokona.nijika.dev") {
                    println!("Failed to open URL: {}", e);
                }
            }
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
                unsafe {
                    if let Some(editor_state) = EDITOR_STATE.as_mut() {
                        editor_state.force_highlight_update();
                    }
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
        let available_height = ui.available_height()
            - (unsafe {
                if TERMINAL_OPEN {
                    260.0
                } else {
                    20.0
                }
            });

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
                            .desired_width(unsafe {
                                SETTINGS
                                    .as_ref()
                                    .map_or(35.0, |s| 35.0 * (s.font_size / 12.0))
                            })
                            .min_size(egui::vec2(35.0, available_height))
                            .interactive(false)
                            .font(egui::FontId::monospace(unsafe {
                                SETTINGS.as_ref().map_or(12.0, |s| s.font_size)
                            }))
                            .horizontal_align(egui::Align::RIGHT),
                    );

                    let text_edit = egui::TextEdit::multiline(text)
                        .desired_width(available_width - 50.0)
                        .min_size(egui::vec2(available_width - 50.0, available_height))
                        .font(egui::TextStyle::Monospace)
                        .lock_focus(true);

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
                        if response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Tab)) {
                            let cursor_pos = text.len();
                            text.insert_str(cursor_pos, "    ");
                            WAS_MODIFIED.store(true, Ordering::SeqCst);
                            ctx.send_viewport_cmd(egui::ViewportCommand::Title(
                                "Kokona | MODIFIED".into(),
                            ));
                            response.request_focus();
                        }
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

                    show_bottom_status_bar(ctx, line, col, text, filename);
                });
            });
    });

    was_modified = WAS_MODIFIED.load(Ordering::SeqCst);
    was_modified
}

fn calculate_cursor_position(text: &str) -> (usize, usize) {
    let mut line = 1;
    let mut col = 1;
    // this still sucks, please someone help
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

fn create_pty() -> Result<portable_pty::PtyPair, Box<dyn std::error::Error>> {
    let pty_system = portable_pty::native_pty_system();
    let pair = pty_system.openpty(portable_pty::PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    })?;
    Ok(pair)
}

fn show_bottom_status_bar(
    ctx: &egui::Context,
    line: usize,
    col: usize,
    text: &str,
    filename: &str,
) {
    egui::TopBottomPanel::bottom("bottom_panel")
        .min_height(unsafe {
            if TERMINAL_OPEN {
                220.0
            } else {
                20.0
            }
        })
        .max_height(unsafe {
            if TERMINAL_OPEN {
                220.0
            } else {
                20.0
            }
        })
        .show(ctx, |ui| {
            unsafe {
                if TERMINAL_OPEN {
                    // Initialize terminal state if needed
                    if TERMINAL_PTY.is_none() {
                        println!("Initializing PTY");
                        if let Ok(pty_pair) = create_pty() {
                            TERMINAL_PTY = Some(pty_pair);
                            TERMINAL_OUTPUT = Some(String::new());
                            TERMINAL_INPUT = Some(String::new());

                            if let Some(pty_pair) = &TERMINAL_PTY {
                                match pty_pair.master.take_writer() {
                                    Ok(writer) => TERMINAL_WRITER = Some(writer),
                                    Err(e) => println!("Failed to get writer: {}", e),
                                };

                                let mut cmd = portable_pty::CommandBuilder::new("/bin/sh");
                                cmd.env("TERM", "dumb");
                                cmd.env("LANG", "en_US.UTF-8");
                                cmd.env("LC_ALL", "en_US.UTF-8");
                                if let Some(parent) = std::path::Path::new(&*filename).parent() {
                                    cmd.cwd(parent);
                                }

                                if let Ok(_child) = pty_pair.slave.spawn_command(cmd) {
                                    println!("Shell started successfully");
                                    let mut reader = pty_pair.master.try_clone_reader().unwrap();

                                    // Read output in a separate thread
                                    std::thread::spawn(move || {
                                        let mut buffer = [0u8; 1024];
                                        loop {
                                            match reader.read(&mut buffer) {
                                                Ok(0) => break,
                                                Ok(n) => {
                                                    let str = String::from_utf8_lossy(&buffer[..n])
                                                        .into_owned();
                                                    unsafe {
                                                        if let Some(output) = &mut TERMINAL_OUTPUT {
                                                            output.push_str(&str);
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    println!("Read error: {}", e);
                                                    break;
                                                }
                                            }
                                        }
                                    });
                                } else {
                                    println!("Failed to start shell");
                                }
                            }
                        } else {
                            println!("Failed to create PTY");
                        }
                    }

                    // Single interactive terminal field
                    ui.vertical(|ui| {
                        let available_width = ui.available_width();

                        if let (Some(output), Some(input)) =
                            (&mut TERMINAL_OUTPUT, &mut TERMINAL_INPUT)
                        {
                            // Combine output and current input line
                            let mut terminal_content = output.clone();
                            if !terminal_content.ends_with('\n') {
                                terminal_content.push('\n');
                            }
                            terminal_content.push_str("$ ");
                            terminal_content.push_str(input);

                            egui::ScrollArea::vertical()
                                .stick_to_bottom(true)
                                .auto_shrink([false; 2])
                                .show(ui, |ui| {
                                    let response = ui.add(
                                        egui::TextEdit::multiline(&mut terminal_content)
                                            .min_size(egui::vec2(available_width, 180.0))
                                            .font(egui::TextStyle::Monospace)
                                            .cursor_at_end(true),
                                    );

                                    // Handle input
                                    if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                        if !input.trim().is_empty() {
                                            if let Some(writer) = &mut TERMINAL_WRITER {
                                                writeln!(writer, "{}", input).unwrap();
                                            }
                                            output.push_str("$ ");
                                            output.push_str(input);
                                            output.push('\n');
                                            input.clear();

                                            // Clear the last line from terminal_content
                                            if let Some(last_line) = terminal_content.lines().last()
                                            {
                                                terminal_content.truncate(
                                                    terminal_content.len() - last_line.len(),
                                                );
                                                if terminal_content.ends_with('\n') {
                                                    terminal_content.push_str("$ ");
                                                }
                                            }
                                        }
                                        response.request_focus();
                                    }

                                    // Update input based on new content
                                    if let Some(last_line) = terminal_content.lines().last() {
                                        if last_line.starts_with("$ ") {
                                            *input = last_line[2..].to_string();
                                        }
                                    }
                                });
                        }

                        // Status bar
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::BOTTOM), |ui| {
                            ui.add_space(5.0);
                            ui.label(format!(
                                "{} lines, {} columns | Characters: {}",
                                line,
                                col,
                                text.len()
                            ));
                        });
                    });
                } else {
                    // Show status bar when terminal is closed
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::BOTTOM), |ui| {
                        ui.add_space(5.0);
                        ui.label(format!(
                            "{} lines, {} columns | Characters: {}",
                            line,
                            col,
                            text.len()
                        ));
                    });

                    // Cleanup terminal when closed
                    if TERMINAL_PTY.is_some() {
                        println!("Cleaning up terminal");
                        TERMINAL_PTY = None;
                        TERMINAL_OUTPUT = None;
                        TERMINAL_INPUT = None;
                        TERMINAL_WRITER = None;
                    }
                }
            }
        });
}
