use eframe::{egui, App, Frame, NativeOptions};

fn main() -> Result<(), eframe::Error> {
    let options = NativeOptions {
        vsync: true,                                       // Enable vsync
        multisampling: 4,
        ..Default::default()
    };

    eframe::run_native(
        "Kokona",
        options,
        Box::new(|_cc| Ok(Box::new(MyApp::default()))), // Updated closure to return Result
    )
}

#[derive(Default)]
struct MyApp;

impl App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                if ui.button("Click me").clicked() {
                    println!("Button clicked!");
                }
            });
        });
    }
}
