//! GitCraft, a graphical Git client: drawn with egui on top of the back end in
//! `gitcraft-core`.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod instance;
#[cfg(target_os = "macos")]
mod macos;
mod settings;

use app::{App, StartOptions};
use std::path::PathBuf;

const HELP: &str = "GitCraft: a graphical Git client

Usage: gitcraft [options] [repository folders...]

Options:
  --screenshot <file.png>   Start with default preferences, save the window as a PNG and exit
  --theme <name>            Use this theme for this run: system, dark, light, dark-hc, light-hc
  --shot-view <view>        With --screenshot: show diff, inline, split, history, blame or palette
  --new-window              Open the repository in a window of its own
  --help                    Show this help
";

fn parse_args() -> Result<StartOptions, String> {
    let mut o = StartOptions::default();
    let cwd = std::env::current_dir().unwrap_or_default();
    let mut it = std::env::args().skip(1);
    let mut rest = false;
    while let Some(a) = it.next() {
        if rest {
            o.launch.extend(launch_intent(&a, &cwd));
            continue;
        }
        match a.as_str() {
            "--" => rest = true,
            "--new-window" => o.new_window = true,
            "--screenshot" => o.screenshot = Some(PathBuf::from(it.next().ok_or("--screenshot needs a file name")?)),
            "--theme" => o.theme = Some(it.next().ok_or("--theme needs a name")?),
            "--shot-view" => o.shot_view = Some(it.next().ok_or("--shot-view needs a view")?),
            "--help" | "-h" => {
                print!("{HELP}");
                std::process::exit(0);
            }
            other if other.starts_with("--") => return Err(format!("Unknown option {other}. Try --help.")),
            other => o.launch.extend(launch_intent(other, &cwd)),
        }
    }
    Ok(o)
}

/// A folder (from the command line or the file manager) or a `gitcraft://clone?url=` link.
fn launch_intent(argument: &str, cwd: &std::path::Path) -> Option<gitcraft_core::session::LaunchIntent> {
    use gitcraft_core::session::{LaunchIntent, clone_launch, explorer_argument};
    if argument.starts_with("gitcraft:") {
        return clone_launch(argument);
    }
    let path = cwd.join(explorer_argument(argument).as_ref());
    if path.is_dir() {
        let path = std::fs::canonicalize(&path).unwrap_or(path);
        Some(LaunchIntent::Repository { path: plain_path(&path.to_string_lossy()) })
    } else {
        eprintln!("Not a folder: {argument}");
        None
    }
}

/// A canonical path in the form people and settings use: on Windows without the
/// extended-length prefix canonicalize adds (`\\?\C:\x` is `C:\x`, `\\?\UNC\s\x` is `\\s\x`).
fn plain_path(path: &str) -> String {
    if !cfg!(windows) {
        return path.to_owned();
    }
    match path.strip_prefix(r"\\?\UNC\") {
        Some(rest) => format!(r"\\{rest}"),
        None => path.strip_prefix(r"\\?\").unwrap_or(path).to_owned(),
    }
}

/// The app icon for the window and task bar, built into the program.
fn window_icon() -> egui::IconData {
    let bytes: &[u8] = include_bytes!("../../../icons/128x128.png");
    let decoded = (|| {
        let mut reader = png::Decoder::new(std::io::Cursor::new(bytes)).read_info().ok()?;
        let mut buf = vec![0u8; reader.output_buffer_size()?];
        let info = reader.next_frame(&mut buf).ok()?;
        (info.color_type == png::ColorType::Rgba).then(|| egui::IconData { rgba: buf[..info.buffer_size()].to_vec(), width: info.width, height: info.height })
    })();
    decoded.unwrap_or_default()
}

#[cfg(target_os = "macos")]
// Call only at startup, before any threads that might access the environment exist.
unsafe fn add_macos_tool_paths() {
    use std::path::Path;
    // Finder uses a system-only PATH. Preserve the user's existing tool order when
    // launched from a shell and place missing package managers before the OS shims.
    let existing = std::env::var_os("PATH").filter(|path| !path.is_empty()).unwrap_or_else(|| "/usr/bin:/bin:/usr/sbin:/sbin".into());
    let mut folders: Vec<_> = std::env::split_paths(&existing).filter(|path| path.is_absolute()).collect();
    let package_managers = if cfg!(target_arch = "aarch64") { ["/opt/homebrew/bin", "/usr/local/bin"] } else { ["/usr/local/bin", "/opt/homebrew/bin"] };
    let missing = package_managers.into_iter().map(PathBuf::from).filter(|path| path.is_dir() && !folders.contains(path)).collect::<Vec<_>>();
    let system = folders.iter().position(|path| path == Path::new("/usr/bin")).unwrap_or(folders.len());
    folders.splice(system..system, missing);
    if let Ok(path) = std::env::join_paths(folders) {
        // SAFETY: The caller guarantees this is still single threaded.
        unsafe { std::env::set_var("PATH", path) };
    }
}

fn main() {
    app::bench::mark("main");
    if gitcraft_core::accounts::handle_ssh_askpass() {
        return;
    }
    #[cfg(target_os = "macos")]
    // SAFETY: The startup steps above do not spawn threads.
    unsafe { add_macos_tool_paths() };
    let opts = match parse_args() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    if opts.screenshot.is_none() {
        gitcraft_core::diagnostics::install();
    }
    let mut opts = opts;
    // A second launch hands its folders to the running window and quits, unless it asks for a
    // theme of its own, which only a window of its own can show.
    if opts.screenshot.is_none() && !opts.new_window && opts.theme.is_none() {
        match instance::claim(&opts.launch) {
            instance::Claim::Forwarded => return,
            instance::Claim::Primary(p) => opts.primary = Some(p),
            instance::Claim::Standalone(why) => eprintln!("Opening a separate GitCraft window: {why}"),
        }
    }

    // Picture-taking runs use the defaults and leave the user's preferences alone.
    let store = if opts.screenshot.is_some() { settings::Store::in_memory() } else { settings::Store::open() };
    let frame = app::frame::decide(store.flag("systemTitleBar", false));
    let viewport = egui::ViewportBuilder::default()
        .with_title("GitCraft")
        .with_inner_size(app::placement::SIZE)
        .with_min_inner_size(app::placement::MIN)
        .with_drag_and_drop(true)
        .with_icon(window_icon());
    // Picture-taking runs keep the default size, so the pictures always match.
    let viewport = if opts.screenshot.is_none() { app::placement::apply(viewport) } else { viewport };
    let mut viewport = app::frame::install(frame, viewport);
    // In a Flatpak the window must carry the app's id, or the desktop can't match it to its icon.
    if let Ok(id) = std::env::var("FLATPAK_ID") {
        viewport = viewport.with_app_id(id);
    }
    #[cfg(target_os = "macos")]
    if opts.screenshot.is_none() {
        macos::handle_links();
    }
    let no_vsync = opts.screenshot.is_some() || app::bench::scrolling();
    if opts.theme.clone().unwrap_or_else(|| store.theme()) == "system" {
        app::theme::prestart();
    }
    // The first repository is read while the window and the graphics are set up.
    let engine = std::sync::Arc::new(gitcraft_core::actions::Engine::default());
    app::repo::prefetch(&engine, &store, &opts.launch, !opts.new_window && opts.screenshot.is_none());
    opts.engine = Some(engine);
    app::bench::mark("window");
    // Vulkan, Metal or DirectX 12 first, which leaves the OpenGL drivers unloaded; OpenGL when
    // none of them works. The app is made only once a window could be set up, so it is still
    // here for the second try.
    let start = std::rc::Rc::new(std::cell::RefCell::new(Some((opts, store))));
    let mut failure = None;
    for backends in [egui_wgpu::wgpu::Backends::PRIMARY, egui_wgpu::wgpu::Backends::GL] {
        if start.borrow().is_none() {
            break;
        }
        let native = eframe::NativeOptions { renderer: eframe::Renderer::Wgpu, wgpu_options: graphics(backends, no_vsync), viewport: viewport.clone(), ..Default::default() };
        let take = start.clone();
        let run = eframe::run_native("GitCraft", native, Box::new(move |cc| {
            let (opts, store) = take.borrow_mut().take().ok_or("GitCraft already started")?;
            Ok(Box::new(App::new(cc, opts, store)))
        }));
        match run {
            Ok(()) => return,
            Err(e) => failure = Some(e),
        }
    }
    if let Some(e) = failure {
        eprintln!("GitCraft could not open its window: {e}\nIt needs a graphics driver with Vulkan, Metal, DirectX 12 or OpenGL.");
        std::process::exit(1);
    }
}

/// How the window draws: on `backends` (unless `WGPU_BACKEND` says otherwise), on the
/// power-saving GPU (`WGPU_POWER_PREF` overrides it; a Git client has no use for waking a
/// laptop's discrete GPU), and with GPU memory allocated as needed rather than in large blocks
/// up front.
fn graphics(backends: egui_wgpu::wgpu::Backends, no_vsync: bool) -> egui_wgpu::WgpuConfiguration {
    use egui_wgpu::wgpu;
    let mut setup = egui_wgpu::WgpuSetupCreateNew::without_display_handle();
    setup.instance_descriptor.backends = wgpu::Backends::from_env().unwrap_or(backends);
    setup.power_preference = wgpu::PowerPreference::from_env().unwrap_or(wgpu::PowerPreference::LowPower);
    let device = setup.device_descriptor.clone();
    setup.device_descriptor = std::sync::Arc::new(move |adapter| wgpu::DeviceDescriptor { memory_hints: wgpu::MemoryHints::MemoryUsage, ..device(adapter) });
    let mut options = egui_wgpu::WgpuConfiguration { wgpu_setup: egui_wgpu::WgpuSetup::CreateNew(setup), ..Default::default() };
    if no_vsync {
        // A picture-taking run, or the scroll benchmark, must not wait on the display.
        options.surface.present_mode = wgpu::PresentMode::AutoNoVsync;
    }
    options
}
