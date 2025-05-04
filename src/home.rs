use crate::AppMsg;
use gtk4::prelude::*;
use relm4::prelude::*;

#[derive(Default, Debug)]
pub struct HomeModel {}

#[derive(Debug)]
pub enum HomeMsg {
    OpenFile,
    NewFile,
}

#[relm4::component(pub)]
impl SimpleComponent for HomeModel {
    type Init = ();
    type Input = HomeMsg;
    type Output = AppMsg;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 10,
            set_margin_all: 20,

            #[name = "new_button"]
            gtk::Button {
                set_label: "New File",
                connect_clicked[sender] => move |_| {
                    sender.output(AppMsg::SwitchToEditor("".to_string())).unwrap();
                }
            },

            #[name = "open_button"]
            gtk::Button {
                set_label: "Open File",
                connect_clicked[sender] => move |_| {
                    sender.output(AppMsg::SwitchToEditor("".to_string())).unwrap();
                }
            }
        }
    }

    fn init(
        _: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = HomeModel::default();
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            HomeMsg::OpenFile => {
                sender
                    .output(AppMsg::SwitchToEditor("".to_string()))
                    .unwrap();
            }
            HomeMsg::NewFile => {
                sender
                    .output(AppMsg::SwitchToEditor("".to_string()))
                    .unwrap();
            }
        }
    }
}
