mod views;
use eframe::{egui, App, Frame, NativeOptions};
use views::ViewType;

#[derive(Default)]
struct MyApp {
    current_view: ViewType,
    opened: String,
    filename: String,
}

impl App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        // filename is now stored in self
        match self.current_view {
            ViewType::Home => views::home_view(
                ctx,
                &mut self.current_view,
                &mut self.filename,
                &mut self.opened,
            ),
            ViewType::Editor => views::editor_view(ctx, &mut self.opened, &mut self.filename),
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Tab)) {
            self.current_view = match self.current_view {
                ViewType::Home => ViewType::Editor,
                ViewType::Editor => ViewType::Home,
            };
        }
    }
}

fn main() -> Result<(), eframe::Error> {
    let options = NativeOptions {
        vsync: true,
        multisampling: 4,
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 720.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Kokona",
        options,
        Box::new(|_cc| Ok(Box::new(MyApp::default()))),
    )
}
