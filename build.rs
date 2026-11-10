use std::path::Path;
use std::process::Command;

// Windows cannot replace an executable while a preview is running. Renaming it
// keeps that preview alive and frees Cargo's output path for the next build.
// Also handle Cargo's hard link in deps/, which refers to the same executable.
#[cfg(windows)]
fn prepare_windows_output() {
    use std::{env, fs, io, path::PathBuf, time::{SystemTime, UNIX_EPOCH}};

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo must set OUT_DIR"));
    let profile = out.ancestors().nth(3).expect("Cargo build output directory");
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let deps = profile.join("deps");
    let mut outputs = vec![profile.join("gitcraft.exe")];
    if let Ok(entries) = fs::read_dir(&deps) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.strip_prefix("gitcraft-").and_then(|n| n.strip_suffix(".exe"))
                .is_some_and(|hash| !hash.is_empty() && hash.bytes().all(|b| b.is_ascii_hexdigit()))
            {
                outputs.push(path);
            }
        }
    }
    for path in outputs {
        match fs::OpenOptions::new().write(true).open(&path) {
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied || matches!(e.raw_os_error(), Some(32 | 33)) => {
                let previous = path.with_extension(format!("exe.previous-{stamp}"));
                fs::rename(&path, &previous).unwrap_or_else(|e| {
                    panic!("Could not move the locked previous build {} aside: {e}. Close that GitCraft preview and rebuild.", path.display())
                });
                println!("cargo:warning=Moved running previous build aside: {}", path.display());
            }
            Err(e) if e.kind() != io::ErrorKind::NotFound => {
                panic!("Could not inspect build output {}: {e}", path.display());
            }
            _ => {}
        }
    }
    // Old previews may have exited since the last build. Delete only the files
    // this helper retired, leaving still-running previews alone.
    for dir in [profile.to_path_buf(), deps] {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.starts_with("gitcraft") && name.contains(".exe.previous-") {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
    }
}

fn main() {
    // Rust changes must run the output preparation too, not just UI changes.
    println!("cargo:rerun-if-changed=src");
    #[cfg(windows)]
    prepare_windows_output();

    // Rebuild the React front end when its sources change, so a plain `cargo build`
    // always embeds the current UI. If npm isn't available the existing ui/dist is used.
    println!("cargo:rerun-if-changed=ui/src");
    println!("cargo:rerun-if-changed=ui/index.html");
    println!("cargo:rerun-if-changed=ui/package.json");
    println!("cargo:rerun-if-changed=ui/package-lock.json");
    println!("cargo:rerun-if-changed=ui/vite.config.ts");
    println!("cargo:rerun-if-changed=ui/tsconfig.json");
    let ui = Path::new("ui");
    if ui.join("node_modules").exists() {
        let npm = if cfg!(windows) { "npm.cmd" } else { "npm" };
        match Command::new(npm).args(["run", "build"]).current_dir(ui).status() {
            Ok(s) if s.success() => {}
            Ok(s) => {
                panic!("front-end build failed ({s}); refusing to embed an outdated ui/dist");
            }
            Err(e) => panic!("could not run npm ({e}); refusing to embed an outdated ui/dist"),
        }
    }
    tauri_build::build()
}
