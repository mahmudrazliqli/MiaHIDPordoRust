fn main() {
    glib_build_tools::compile_resources(
        &["resources"],
        "resources/resources.gresource.xml",
        "miahidpordo.gresource",
    );

    println!("cargo:rerun-if-changed=resources/resources.gresource.xml");
    println!("cargo:rerun-if-changed=resources/window1.glade");
    println!("cargo:rerun-if-changed=resources/style.css");
}
