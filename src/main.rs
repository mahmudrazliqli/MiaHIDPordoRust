#![allow(deprecated)]
mod app;
mod config;
mod descriptor;
mod device;
mod reader;
mod sync_op;
mod report_tab;
mod ui;

use gtk::prelude::*;

fn load_resources() {
    // glib-build-tools فایل را در OUT_DIR/miahidpordo.gresource می‌سازد
    let path = concat!(env!("OUT_DIR"), "/miahidpordo.gresource");
    let resource = gtk::gio::Resource::load(path)
        .expect("Failed to load gresource file");
    gtk::gio::resources_register(&resource);
}

fn main() {
    load_resources();

    let application = gtk::Application::new(
        Some("org.miahidpordo.MiaHidPordo"),
        Default::default(),
    );

    application.connect_activate(|app| {
        ui::build_ui(app);
    });

    application.run();
}
