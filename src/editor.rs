use crate::misc::build_menu;
use gtk4::prelude::*;
use relm4::prelude::*;
use sourceview5::prelude::*;

#[derive(Default, Debug)]
pub struct EditorModel {
    text: String,
    line: i32,
    column: i32,
    char_count: i32,
}

#[derive(Debug)]
pub enum EditorMsg {
    TextChanged(String),
    CursorMoved(i32, i32, i32),
    Save,
    SaveAs,
    Quit,
    About,
}

#[relm4::component(pub)]
impl SimpleComponent for EditorModel {
    type Init = String;
    type Input = EditorMsg;
    type Output = ();

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 0,

            gtk::PopoverMenuBar::from_model(Some(&{
                let menu: gtk4::gio::MenuModel = build_menu().into();
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
            },

            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,

                #[name = "status_label"]
                gtk::Label {
                    set_hexpand: true,
                    set_xalign: 1.0,
                    set_text: &format!("Line {}, Column {} | Characters: {}",
                        model.line, model.column, model.char_count),
                    set_margin_end: 10,
                    set_margin_bottom: 5,
                    set_margin_top: 5,
                }
            }
        }
    }

    fn init(
        content: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = EditorModel {
            text: content,
            line: 1,
            column: 1,
            char_count: 0,
        };

        let widgets = view_output!();

        let buffer = sourceview5::Buffer::new(None);
        let style_manager = sourceview5::StyleSchemeManager::default();
        if let Some(scheme) = style_manager.scheme("Adwaita-dark") {
            buffer.set_style_scheme(Some(&scheme));
        }

        widgets.source_view.set_buffer(Some(&buffer));

        {
            let sender_clone = sender.clone();
            let status_label = widgets.status_label.clone();
            buffer.connect_changed(move |buffer| {
                let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
                let char_count = text.as_str().chars().count() as i32;

                let cursor_iter = buffer.iter_at_mark(&buffer.get_insert());
                let line = cursor_iter.line() + 1;
                let column = cursor_iter.line_offset() + 1;

                sender_clone.input(EditorMsg::CursorMoved(line, column, char_count));

                status_label.set_text(&format!(
                    "Line {}, Column {} | Characters: {}",
                    line, column, char_count
                ));
            });
        }

        {
            let sender_clone = sender.clone();
            let status_label = widgets.status_label.clone();
            buffer.connect_mark_set(move |buffer, iter, mark| {
                if mark.name().as_deref() == Some("insert") {
                    let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
                    let char_count = text.as_str().chars().count() as i32;

                    let line = iter.line() + 1;
                    let column = iter.line_offset() + 1;

                    sender_clone.input(EditorMsg::CursorMoved(line, column, char_count));

                    status_label.set_text(&format!(
                        "Line {}, Column {} | Characters: {}",
                        line, column, char_count
                    ));
                }
            });
        }

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            EditorMsg::TextChanged(text) => {
                self.text = text;
            }
            EditorMsg::CursorMoved(line, column, char_count) => {
                self.line = line;
                self.column = column;
                self.char_count = char_count;
            }
            EditorMsg::Save => {
                println!("Save file");
            }
            EditorMsg::SaveAs => {
                println!("Save As");
            }
            EditorMsg::Quit => {
                println!("Quitting...");
                std::process::exit(0);
            }
            EditorMsg::About => {
                let about = gtk4::AboutDialog::builder()
                    .program_name("Kokona")
                    .version("0.1.0")
                    .authors(
                        vec!["Eri"]
                            .into_iter()
                            .map(String::from)
                            .collect::<Vec<_>>(),
                    )
                    .comments("A simple GTK text editor")
                    .logo_icon_name("text-editor")
                    .modal(true)
                    .build();

                about.present();
            }
        }
    }
}
