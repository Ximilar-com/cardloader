// Prevents an additional console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // `cardloader mcp`: what Claude Desktop and other agents start; it talks to the running app.
    if std::env::args().nth(1).as_deref() == Some("mcp") {
        std::process::exit(cardloader_lib::mcp_bridge());
    }
    cardloader_lib::run()
}
