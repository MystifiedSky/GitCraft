use std::path::Path;
use std::process::Command;

fn main() {
    // Rebuild the React front end when its sources change, so a plain `cargo build`
    // always embeds the current UI. If npm isn't available the existing ui/dist is used.
    println!("cargo:rerun-if-changed=ui/src");
    println!("cargo:rerun-if-changed=ui/index.html");
    println!("cargo:rerun-if-changed=ui/package.json");
    let ui = Path::new("ui");
    if ui.join("node_modules").exists() {
        let npm = if cfg!(windows) { "npm.cmd" } else { "npm" };
        match Command::new(npm).args(["run", "build"]).current_dir(ui).status() {
            Ok(s) if s.success() => {}
            Ok(s) => {
                if !ui.join("dist").join("index.html").exists() {
                    panic!("front-end build failed ({s})");
                }
                println!("cargo:warning=front-end build failed ({s}); using the previous ui/dist");
            }
            Err(e) => println!("cargo:warning=could not run npm ({e}); using the existing ui/dist"),
        }
    }
    tauri_build::build()
}
