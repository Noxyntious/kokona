use gtk4::gio;

pub fn build_menu() -> gio::Menu {
    let menu = gio::Menu::new();
    let main_menu = gio::Menu::new();
    let file_menu = gio::Menu::new();
    let help_menu = gio::Menu::new();

    main_menu.append_item(&gio::MenuItem::new(Some("Open"), Some("app.open")));
    main_menu.append_item(&gio::MenuItem::new(Some("Save"), Some("app.save")));
    main_menu.append_item(&gio::MenuItem::new(Some("Save As"), Some("app.saveas")));
    main_menu.append_item(&gio::MenuItem::new(Some("Exit"), Some("app.exit")));

    file_menu.append_item(&gio::MenuItem::new(Some("Nothing yet..."), Some("app.idk")));

    help_menu.append_item(&gio::MenuItem::new(Some("About"), Some("app.about")));

    menu.append_submenu(Some("Kokona"), &main_menu);
    menu.append_submenu(Some("File"), &file_menu);
    menu.append_submenu(Some("Git"), &file_menu);
    menu.append_submenu(Some("Help"), &help_menu);

    menu
}
