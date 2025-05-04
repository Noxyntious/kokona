use gtk4::gio;
use gtk4::prelude::*;
use relm4::prelude::*;
use sourceview5::prelude::*;

mod editor;
mod home;
mod misc;

use crate::editor::EditorModel;
use crate::home::HomeModel;

#[derive(Debug, Default)]
pub enum View {
    #[default]
    Home,
    Editor(String),
}

#[derive(Default)]
pub struct AppModel {
    current_view: View,
}

#[derive(Debug)]
pub enum AppMsg {
    SwitchToEditor(String),
    SwitchToHome,
}

#[relm4::component(pub)]
impl SimpleComponent for AppModel {
    type Init = ();
    type Input = AppMsg;
    type Output = ();

    view! {
        main_window = gtk::Window {
            set_title: Some("Kokona"),
            set_default_size: (1280, 720),

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,

                if matches!(model.current_view, View::Home) {
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 10,
                        set_margin_all: 20,

                        gtk::Button {
                            set_label: "New File",
                            connect_clicked[sender] => move |_| {
                                sender.input(AppMsg::SwitchToEditor("".to_string()));
                            }
                        },

                        gtk::Button {
                            set_label: "Open File",
                            connect_clicked[sender] => move |_| {
                                sender.input(AppMsg::SwitchToEditor("".to_string()));
                            }
                        }
                    }
                } else if let View::Editor(content) = &model.current_view {
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 0,

                        gtk::PopoverMenuBar::from_model(Some(&{
                            let menu: gtk4::gio::MenuModel = misc::build_menu().into();
                            menu
                        })) {},

                        #[name = "source_view"]
                        sourceview5::View {
                            set_vexpand: true,
                            set_hexpand: true,
                            set_wrap_mode: gtk::WrapMode::WordChar,
                            set_show_line_numbers: true,
                            set_highlight_current_line: true,
                            set_monospace: true,
                            set_background_pattern: sourceview5::BackgroundPatternType::None,
                            set_buffer: Some(&{
                                let buffer = sourceview5::Buffer::new(None);
                                let style_manager = sourceview5::StyleSchemeManager::default();
                                if let Some(scheme) = style_manager.scheme("Adwaita-dark") {
                                    buffer.set_style_scheme(Some(&scheme));
                                }
                                buffer
                            }),
                        },

                        gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,

                            gtk::Label {
                                set_hexpand: true,
                                set_xalign: 1.0,
                                set_text: "Line 1, Column 1 | Characters: 0",
                                set_margin_end: 10,
                                set_margin_bottom: 5,
                                set_margin_top: 5,
                            }
                        }
                    }
                } else {
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        append = &gtk::Label {
                            set_label: "Unknown View"
                        }
                    }
                }
            }
        }
    }

    fn init(
        _: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = AppModel::default();
        let widgets = view_output!();

        if let View::Editor(_) = model.current_view {
            let buffer = sourceview5::Buffer::new(None);
            let style_manager = sourceview5::StyleSchemeManager::default();
            if let Some(scheme) = style_manager.scheme("Adwaita-dark") {
                buffer.set_style_scheme(Some(&scheme));
            }
            widgets.source_view.set_buffer(Some(&buffer));
        }

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            AppMsg::SwitchToEditor(content) => {
                self.current_view = View::Editor(content);
            }
            AppMsg::SwitchToHome => {
                self.current_view = View::Home;
            }
        }
    }
}

fn main() {
    let app = RelmApp::new("dev.eri.kokona");
    app.run::<AppModel>(());
}
