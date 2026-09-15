fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("assets/logo.ico")
            .set("ProductName", "v_tileworkizer")
            .set("FileDescription", "v_tileworkizer workspace organizer")
            .compile()
            .expect("embed Windows icon and version information");
    }
}
