//! Slide desktop app (the personal fork of DeckCraft).
//!
//! Usage: `slide [--control <port>] [--sample] [--show] [files…]`
//! A PowerPoint Show (`.ppsx`) given last starts its slide show, as a double-click does in PowerPoint.
//! `deckcraft --help` prints the flags and exits. `deckcraft --version` prints the version and exits.
//!
//! `--control <port>` (or `DECKCRAFT_CONTROL_PORT`) starts a localhost JSON-lines control server:
//! `{"id":1,"method":"ui.inspect","params":{}}` → `{"id":1,"ok":true,"result":…}`.
//! See `deckcraft_ui_egui::control` for the methods.
//!
//! `log` records go to standard error and `<settings dir>/logs/deckcraft.log` (`RUST_LOG` sets the
//! levels); see [`logging`].
#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

#[cfg(target_os = "macos")]
mod apple_events;
mod audio;
mod control_server;
#[cfg(all(unix, not(target_os = "macos")))]
mod display;
#[cfg(any(target_os = "windows", test))]
mod graphics;
mod logging;
#[cfg(not(target_arch = "wasm32"))]
mod pasteboard;

use deckcraft_engine::Session;
use deckcraft_ui_egui::{Services, SlideApp};

struct App(
    SlideApp,
    /// Files opened from Finder and quit requests (see `apple_events`).
    #[cfg(target_os = "macos")]
    fmv_macos_events::Inbox,
);

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        #[cfg(target_os = "macos")]
        apple_events::poll(&self.1, &mut self.0, ctx);
        self.0.logic(ctx);
    }
    /// Forward native raw input to the wrapped application once per pass.
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        self.0.raw_input_hook(ctx, raw);
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.0.ui(ui);
        // An update's helper waits for the new Slide's first frame before it calls the update done.
        deckcraft_update::helper::acknowledge_startup();
    }
    fn on_exit(&mut self) {
        save_prefs(&self.0);
        // The window is closing for good: after Restart to Update (and its quit prompt), the
        // update's helper starts now and installs it once the process has exited.
        self.0.start_pending_update();
        // macOS (#279): end the process here, before eframe tears the window down. AccessKit's
        // macOS adapter subclasses the window's content view and restores its class on teardown;
        // AppKit's Touch Bar support observes that view (KVO), so in the next display cycle
        // `-[_NSTouchBarFinderObservation invalidate]` throws "not registered" and the quit ends
        // in a crash report. eframe calls `on_exit` on every way of quitting (⌘Q, the close
        // button, `app.quit`), once the unsaved-changes prompt has let the window close, and
        // nothing after it is needed: the preferences are saved and the log file is unbuffered.
        // `on_exit` only runs once the app exists, never on a failed start. Same as GridCraft #117.
        #[cfg(target_os = "macos")]
        self.0.cleanup_print_previews();
        #[cfg(target_os = "macos")]
        std::process::exit(0);
    }
}

/// The per-user settings directory: `ui.json`, `prefs.json` and `logs/` live here.
fn config_dir() -> Option<std::path::PathBuf> {
    let var = |k| std::env::var_os(k).map(std::path::PathBuf::from);
    settings_dir(cfg!(target_os = "macos"), cfg!(windows), var("HOME"), var("APPDATA"), var("XDG_CONFIG_HOME"))
}

/// Slide's own folder, apart from upstream DeckCraft's (`DeckCraft`, `deckcraft`): the
/// environment is read by [`config_dir`] so the layout can be tested.
fn settings_dir(
    macos: bool,
    windows: bool,
    home: Option<std::path::PathBuf>,
    appdata: Option<std::path::PathBuf>,
    xdg_config_home: Option<std::path::PathBuf>,
) -> Option<std::path::PathBuf> {
    if macos {
        home.map(|h| h.join("Library/Application Support").join(deckcraft_ui_egui::APP_NAME))
    } else if windows {
        appdata.map(|a| a.join(deckcraft_ui_egui::APP_NAME))
    } else {
        xdg_config_home.or_else(|| home.map(|h| h.join(".config"))).map(|c| c.join(deckcraft_ui_egui::APP_NAME.to_lowercase()))
    }
}

/// AutoRecover files and the user dictionary live under Slide's folder, not upstream DeckCraft's
/// (the engine's own defaults), so Slide never picks up or deletes the original's recovered work.
fn recovery_dir_in(config: &std::path::Path) -> std::path::PathBuf {
    config.join("Recovery")
}

fn user_dictionary_in(config: &std::path::Path) -> std::path::PathBuf {
    config.join("Dictionaries").join("user.dic")
}

fn load_prefs(app: &mut SlideApp) {
    if std::env::var_os("DECKCRAFT_NO_PREFS").is_some() {
        return;
    }
    let Some(dir) = config_dir() else { return };
    let (ui, prefs) = deckcraft_ui_egui::settings::load(&dir);
    if let Some(ui) = ui {
        app.ui = ui;
    }
    if let Some(prefs) = prefs {
        app.session.prefs = prefs;
    }
}

fn save_prefs(app: &SlideApp) {
    if std::env::var_os("DECKCRAFT_NO_PREFS").is_some() {
        return;
    }
    if let Some(dir) = config_dir() {
        deckcraft_ui_egui::settings::save(&dir, &app.ui, &app.session.prefs);
    }
}

/// The desktop services. `native_wayland`: the window is a Wayland client (not Xwayland).
fn services(native_wayland: bool) -> Services {
    // On native Wayland, egui-winit owns the clipboard through winit's data device, which carries
    // text and PNG but no HTML. `pasteboard` (arboard, built for X11 only) would put a copy on the
    // Xwayland selection instead and skip egui's Wayland claim, so Ctrl+V would read something
    // else. There, text goes through egui and pictures arrive as `PasteImage` events.
    let pasteboard = !native_wayland;
    Services {
        pick_open: Some(Box::new(|purpose: &str| {
            let d = rfd::FileDialog::new();
            // The file type names in the interface language (asked from the UI thread, where the
            // language is set); "DeckCraft" and "PowerPoint" are names and have no row.
            let f = |name| deckcraft_ui_egui::i18n::tc("file", name);
            let d = match purpose {
                "picture" => d.add_filter(f("Pictures"), &["png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff"]),
                "audio" => d.add_filter(
                    f("Audio"),
                    &["wav", "mp3", "m4a", "m4b", "aac", "flac", "ogg", "oga", "opus", "aif", "aiff", "aifc", "caf", "wma", "weba", "mka"],
                ),
                "video" => d.add_filter(f("Video"), &["mp4", "m4v", "mov", "webm", "mkv", "wmv"]),
                "theme" => d.add_filter(f("Themes and presentations"), &["thmx", "pptx", "potx", "ppsx"]).add_filter(f("Office Theme"), &["thmx"]),
                _ => d
                    .add_filter(f("Presentations"), &["deckcraft", "pptx", "potx", "ppsx"])
                    .add_filter(f("DeckCraft"), &["deckcraft"])
                    .add_filter(f("PowerPoint"), &["pptx", "potx", "ppsx"])
                    .add_filter(f("Outline"), &["txt", "md"]),
            };
            d.pick_file().map(|p| p.to_string_lossy().to_string())
        })),
        pick_save: Some(Box::new(|name: &str| rfd::FileDialog::new().set_file_name(name).save_file().map(|p| p.to_string_lossy().to_string()))),
        read: Some(Box::new(|p: &str| std::fs::read(p).map_err(|e| e.to_string()))),
        write: Some(Box::new(|p: &str, b: &[u8]| std::fs::write(p, b).map_err(|e| e.to_string()))),
        clipboard_image: pasteboard.then(|| Box::new(pasteboard::image_png) as Box<dyn FnMut() -> Option<Vec<u8>>>),
        clipboard_html: pasteboard.then(|| Box::new(pasteboard::html) as Box<dyn FnMut() -> Option<String>>),
        clipboard_set_html: pasteboard.then(|| Box::new(pasteboard::set_html) as Box<dyn FnMut(&str, &str) -> bool>),
        // Media is mixed and clocked by the output device; `DECKCRAFT_NO_AUDIO` plays silently.
        audio_out: std::env::var_os("DECKCRAFT_NO_AUDIO")
            .is_none()
            .then(|| Box::new(audio::CpalOut::default()) as Box<dyn deckcraft_media::AudioOut>),
        ..Default::default()
    }
}

/// Displays winit lists for `window`, counting an output Wayland lists twice once. `None` when it
/// lists none: Presenter View then stays as it was.
#[cfg(all(unix, not(target_os = "macos")))]
fn display_count(window: &winit::window::Window) -> Option<usize> {
    let n = window.available_monitors().collect::<std::collections::BTreeSet<_>>().len();
    (n > 0).then_some(n)
}

const APP_ID: &str = "io.github.jeongeee.slide";

const USAGE: &str = "\
Usage: slide [--control PORT] [--sample] [--show] [FILE...]

  --control PORT  JSON control server on 127.0.0.1:PORT
  --sample        open the sample presentation
  --show          start the slide show
  --help, -h      print this help and exit
  --version, -V   print the version and exit

A PowerPoint Show (.ppsx) given last starts its slide show.
";

enum Startup {
    Help,
    Version,
    Run { control_port: Option<u16>, files: Vec<String>, sample: bool, show: bool },
}

/// `--help` and `--version` leave before the window and the log file.
fn startup(mut control_port: Option<u16>, args: impl IntoIterator<Item = String>) -> Startup {
    let mut files = Vec::new();
    let mut sample = false;
    let mut show = false;
    let mut args = args.into_iter();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--control" => control_port = args.next().and_then(|p| p.parse().ok()),
            "--sample" => sample = true,
            "--show" => show = true,
            "--version" | "-V" => return Startup::Version,
            "--help" | "-h" => return Startup::Help,
            // The process serial number older macOS adds when Finder launches the app.
            a if a.starts_with("-psn_") => {}
            _ => files.push(a),
        }
    }
    Startup::Run { control_port, files, sample, show }
}

fn app_icon() -> Option<egui::IconData> {
    let png: &[u8] = include_bytes!("../../../assets/app-icon/hicolor/256x256/apps/ai.storyteller.deckcraft.png");
    eframe::icon_data::from_png_bytes(png).map_err(|e| log::warn!("app icon: {e}")).ok()
}

/// Install logging, parse startup arguments and environment, launch the native app, and
/// propagate an eframe startup failure.
fn main() -> eframe::Result {
    // First, so every start-up record (and the engine's panic hook, installed with the first
    // Session) is captured; see `logging`.
    let logger = logging::install();
    // The update helper is a copy of this program started with `--slide-update-helper`: it installs
    // the verified update once this Slide has exited, starts the new one and exits.
    if let Some(done) = deckcraft_update::helper::finish(&std::env::args_os().skip(1).collect::<Vec<_>>()) {
        if let Err(e) = done {
            eprintln!("{} update helper failed: {e}", deckcraft_ui_egui::APP_NAME);
            std::process::exit(1);
        }
        return Ok(());
    }
    let (control_port, files, sample, show) =
        match startup(std::env::var("DECKCRAFT_CONTROL_PORT").ok().and_then(|p| p.parse().ok()), std::env::args().skip(1)) {
            Startup::Help => {
                print!("{USAGE}");
                return Ok(());
            }
            Startup::Version => {
                println!("deckcraft {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            Startup::Run { control_port, files, sample, show } => (control_port, files, sample, show),
        };
    log::info!("{} {} ({} {})", deckcraft_ui_egui::APP_NAME, env!("CARGO_PKG_VERSION"), std::env::consts::OS, std::env::consts::ARCH);
    // The log file lives under the settings directory; opened after the arguments, so `--help` and
    // `--version` leave no file behind. Records logged until now are written to it first.
    if let Some(logger) = logger {
        match config_dir().map(|d| logger.attach_dir(&d.join(logging::LOG_DIR))) {
            Some(Ok(path)) => log::info!("log file {}", path.display()),
            Some(Err(e)) => log::warn!("no log file: {e}"),
            None => {
                logger.stderr_only();
                log::warn!("no log file: no settings directory (HOME, XDG_CONFIG_HOME or APPDATA is not set)");
            }
        }
    }
    // `auto` (the default interface language) follows the system's preferred languages.
    deckcraft_ui_egui::i18n::detect_system_lang();
    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(deckcraft_ui_egui::APP_NAME)
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([900.0, 560.0])
            .with_drag_and_drop(true)
            .with_fullsize_content_view(true)
            .with_titlebar_shown(false)
            .with_title_shown(false),
        ..Default::default()
    };
    options.viewport = options.viewport.with_app_id(APP_ID);
    if let Some(icon) = app_icon() {
        options.viewport = options.viewport.with_icon(icon);
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    let x11 = match display::choose_from_env() {
        display::Choice::X11 => true,
        display::Choice::Default => false,
        display::Choice::None => {
            // winit's own error says little to someone who started Slide from a console.
            log::error!("no display: DISPLAY and WAYLAND_DISPLAY are not set");
            eprintln!("{} needs a graphical session: neither DISPLAY nor WAYLAND_DISPLAY is set.", deckcraft_ui_egui::APP_NAME);
            false
        }
    };
    // `Default` lets winit choose, and winit takes Wayland whenever `WAYLAND_DISPLAY` is set.
    #[cfg(all(unix, not(target_os = "macos")))]
    let native_wayland = !x11 && std::env::var_os("WAYLAND_DISPLAY").is_some_and(|w| !w.is_empty());
    #[cfg(not(all(unix, not(target_os = "macos"))))]
    let native_wayland = false;
    #[cfg(all(unix, not(target_os = "macos")))]
    if x11 {
        options.event_loop_builder = Some(Box::new(|b| {
            use winit::platform::x11::EventLoopBuilderExtX11;
            b.with_x11();
        }));
    }
    // Before eframe creates the wgpu instance: default Windows to DirectX 12 with FXC (see graphics.rs).
    #[cfg(target_os = "windows")]
    graphics::configure(&mut options, eframe::wgpu::Backends::from_env(), eframe::wgpu::Dx12Compiler::from_env());
    // Registered before the event loop starts, so it catches the Finder event that launched us as
    // well as later ones. Lives until the event loop returns; the app creator only borrows it.
    #[cfg(target_os = "macos")]
    let apple_events = apple_events::AppleEvents::install();
    #[cfg(target_os = "macos")]
    let apple_events = &apple_events;
    eframe::run_native(
        deckcraft_ui_egui::APP_NAME,
        options,
        Box::new(move |cc| {
            deckcraft_ui_egui::install_ime_pass_hooks(&cc.egui_ctx);
            let mut app = SlideApp::new(Session::new(), services(native_wayland));
            // Presenter View asks winit how many displays there are when a show starts.
            #[cfg(all(unix, not(target_os = "macos")))]
            if let Some(window) = cc.winit_window().cloned() {
                app.services.displays = Some(Box::new(move || display_count(&window)));
            }
            load_prefs(&mut app);
            if let Some(dir) = config_dir() {
                deckcraft_engine::spell::set_user_dictionary_path(user_dictionary_in(&dir));
            }
            app.enable_update_checks();
            // AutoRecover: reopen what a previous run left unsaved, then keep it current.
            if std::env::var_os("DECKCRAFT_NO_RECOVERY").is_none() {
                app.session.recovery_dir = config_dir().map(|d| recovery_dir_in(&d));
                if let Ok(v) = app.session.execute("file.recovery.open", &serde_json::json!({}))
                    && let Some(n) = v.get("opened").and_then(|o| o.as_array()).map(Vec::len).filter(|n| *n > 0)
                {
                    app.set_status(format!("Recovered {n} unsaved presentation{} from the last session", if n == 1 { "" } else { "s" }));
                }
            }
            app.integrated_titlebar = cfg!(target_os = "macos");
            if let Some(port) = control_port {
                let rx = control_server::start(port, cc.egui_ctx.clone());
                app = app.with_control(rx);
            }
            if sample && let Err(e) = deckcraft_engine::sample::open_sample(&mut app.session) {
                log::error!("sample: {e}");
            }
            // A .ppsx opened last starts its slide show (#862).
            for (f, e) in app.open_from_system(&files) {
                log::error!("{f}: {e}");
            }
            if show {
                app.start_show(0, false);
            }
            Ok(Box::new(App(
                app,
                #[cfg(target_os = "macos")]
                apple_events.connect(&cc.egui_ctx),
            )))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The system Open dialog's file type names are shown in Korean in a Korean interface: every
    /// `add_filter` takes its name through `f` (`tc("file", …)`), and each name but the two
    /// product names has a row. English stays the same text.
    #[test]
    fn open_dialog_file_types_are_korean() {
        use deckcraft_ui_egui::i18n;
        let calls: Vec<&str> = include_str!("main.rs").split(concat!("add_filter", "(")).skip(1).map(str::trim_start).collect();
        let names: Vec<&str> = calls.iter().filter_map(|c| c.strip_prefix("f(\"")?.split('"').next()).collect();
        let heads: Vec<String> = calls.iter().map(|c| c.chars().take(40).collect()).collect();
        assert_eq!(names.len(), calls.len(), "a file type name not given through f(): {heads:?}");
        assert_eq!(names.len(), 9, "{names:?}");
        i18n::set_current(i18n::resolve("ko"));
        for name in &names {
            let shown = i18n::tc("file", name);
            if ["DeckCraft", "PowerPoint"].contains(name) {
                assert_eq!(shown, *name);
            } else {
                assert!(shown.chars().any(|c| ('가'..='힣').contains(&c)), "{name:?} has a Korean row: {shown:?}");
            }
        }
        assert_eq!(i18n::tc("file", "Outline"), "개요", "the text outline, not a shape's");
        i18n::set_current(i18n::resolve("en"));
        assert!(names.iter().all(|n| i18n::tc("file", n) == *n));
    }

    /// The desktop app's own status sentences (AutoRecover, a file the system asked to open) show
    /// in Korean through the status bar's templates, the file name as written.
    #[test]
    fn desktop_statuses_are_korean() {
        use deckcraft_ui_egui::i18n;
        i18n::set_current(i18n::resolve("ko"));
        for n in [1, 2] {
            let s = format!("Recovered {n} unsaved presentation{} from the last session", if n == 1 { "" } else { "s" });
            assert_eq!(i18n::t_dynamic(&s), format!("지난 세션에서 저장하지 않은 프레젠테이션 {n}개를 복구했습니다"));
        }
        assert_eq!(i18n::t_dynamic("Couldn't open /a/Plan.pptx: boom"), "/a/Plan.pptx을(를) 열 수 없습니다: boom");
        i18n::set_current(i18n::resolve("en"));
    }

    /// The fork keeps its settings, prefs and logs apart from upstream DeckCraft's.
    #[test]
    fn recovery_and_dictionary_stay_in_slides_folder() {
        let p = |s: &str| Some(std::path::PathBuf::from(s));
        let upstream = deckcraft_engine::recovery::default_dir();
        for cfg in [
            settings_dir(true, false, p("/Users/a"), None, None),
            settings_dir(false, true, None, p("C:/Roaming"), None),
            settings_dir(false, false, p("/home/a"), None, None),
        ] {
            let cfg = cfg.unwrap();
            assert!(recovery_dir_in(&cfg).starts_with(&cfg) && recovery_dir_in(&cfg).ends_with("Recovery"));
            assert!(user_dictionary_in(&cfg).starts_with(&cfg) && user_dictionary_in(&cfg).ends_with("Dictionaries/user.dic"));
            assert_ne!(Some(recovery_dir_in(&cfg)), upstream);
            assert!(!recovery_dir_in(&cfg).to_string_lossy().contains("DeckCraft"));
            assert!(!user_dictionary_in(&cfg).to_string_lossy().contains("DeckCraft"));
        }
    }

    #[test]
    fn config_dir_is_slides_own() {
        use std::path::PathBuf;
        let p = |s: &str| Some(PathBuf::from(s));
        assert_eq!(settings_dir(true, false, p("/Users/a"), None, None), p("/Users/a/Library/Application Support/Slide"));
        assert_eq!(settings_dir(false, true, None, p("C:/Roaming"), None), p("C:/Roaming/Slide"));
        assert_eq!(settings_dir(false, false, p("/home/a"), None, None), p("/home/a/.config/slide"));
        assert_eq!(settings_dir(false, false, p("/home/a"), None, p("/xdg")), p("/xdg/slide"));
        assert_eq!(settings_dir(false, false, None, None, None), None);
    }

    /// #279: on macOS the process ends inside `on_exit`, before eframe would tear the window down.
    /// The check runs in a child (this test binary again), since on macOS it ends the process.
    #[test]
    fn on_exit_ends_the_process_on_macos() {
        if std::env::var_os("DECKCRAFT_TEST_ON_EXIT").is_some() {
            let mut app = App(
                SlideApp::new(Session::new(), Services::default()),
                #[cfg(target_os = "macos")]
                fmv_macos_events::Inbox::default(),
            );
            println!("calling on_exit");
            eframe::App::on_exit(&mut app);
            println!("on_exit returned");
            return;
        }
        let out = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "tests::on_exit_ends_the_process_on_macos", "--nocapture"])
            .env("DECKCRAFT_TEST_ON_EXIT", "1")
            .env("DECKCRAFT_NO_PREFS", "1")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(out.status.success(), "{stdout}");
        assert!(stdout.contains("calling on_exit"), "the child didn't run the test: {stdout}");
        assert_eq!(stdout.contains("on_exit returned"), !cfg!(target_os = "macos"), "{stdout}");
    }

    /// The egui 0.37 migration (#1350) left `clipboard` out of eframe's features. egui then
    /// simulated a clipboard inside the app: nothing pasted from other apps, and Ctrl+C/Ctrl+V
    /// lost text that went out as HTML. Keep the OS clipboard on.
    #[test]
    fn eframe_uses_the_os_clipboard() {
        let manifest = include_str!("../Cargo.toml");
        let eframe = manifest.lines().find(|l| l.starts_with("eframe = ")).unwrap_or_default();
        assert!(eframe.contains("\"clipboard\""), "eframe must enable the OS clipboard: {eframe}");
    }

    /// On native Wayland, the X11-only pasteboard would bypass egui's Wayland clipboard; elsewhere
    /// it carries HTML and pictures beside egui's text.
    #[test]
    fn the_pasteboard_stays_off_native_wayland() {
        let s = services(true);
        assert!(s.clipboard_html.is_none() && s.clipboard_set_html.is_none() && s.clipboard_image.is_none());
        let s = services(false);
        assert!(s.clipboard_html.is_some() && s.clipboard_set_html.is_some() && s.clipboard_image.is_some());
    }

    #[test]
    fn help_exits_instead_of_opening_a_file() {
        assert!(matches!(startup(None, ["--help".to_string()]), Startup::Help));
        assert!(matches!(startup(None, ["-h".to_string()]), Startup::Help));
        assert!(matches!(startup(None, ["talk.pptx".to_string(), "--help".to_string()]), Startup::Help));
        assert!(matches!(startup(None, ["--version".to_string()]), Startup::Version));
        let launch = startup(Some(9), ["--sample".to_string(), "--control".to_string(), "4242".to_string(), "talk.pptx".to_string()]);
        assert!(matches!(launch, Startup::Run { .. }));
        if let Startup::Run { control_port, files, sample, show } = launch {
            assert_eq!(control_port, Some(4242));
            assert_eq!(files, ["talk.pptx".to_string()]);
            assert!(sample);
            assert!(!show);
        }
    }

    #[test]
    fn finder_process_serial_number_is_not_a_file() {
        let launch = startup(None, ["-psn_0_1234567".to_string(), "talk.pptx".to_string()]);
        assert!(matches!(launch, Startup::Run { .. }));
        if let Startup::Run { files, .. } = launch {
            assert_eq!(files, ["talk.pptx".to_string()]);
        }
    }

    /// The `<string>` values of every `<array>` that follows a `<key>{key}</key>` line.
    fn plist_strings(plist: &str, key: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut in_array = false;
        for line in plist.lines() {
            let line = line.trim();
            if line == format!("<key>{key}</key>") {
                in_array = true;
                continue;
            }
            if in_array {
                for part in line.split("<string>").skip(1) {
                    if let Some((s, _)) = part.split_once("</string>") {
                        out.push(s.to_string());
                    }
                }
                if line.ends_with("</array>") {
                    in_array = false;
                }
            }
        }
        out
    }

    /// Every desktop offers the same file types: macOS (`CFBundleTypeExtensions` and the native
    /// type's extension in `Info.plist.in`), Windows (`SupportedTypes` in `deckcraft.wxs`) and
    /// Linux (`MimeType` in the desktop file). A format added to one installer must reach the
    /// others, and each must be one Finder, Explorer or the file manager can hand to DeckCraft.
    #[test]
    fn every_desktop_offers_the_same_file_types() {
        let plist = include_str!("../../../packaging/macos/Info.plist.in");
        let wxs = include_str!("../../../packaging/windows/deckcraft.wxs");
        let desktop = include_str!("../../../packaging/linux/ai.storyteller.deckcraft.desktop");

        let mut mac = plist_strings(plist, "CFBundleTypeExtensions");
        // The native type is declared by its UTI; its extension is in the exported type.
        mac.extend(plist_strings(plist, "public.filename-extension").into_iter().filter(|e| e == "deckcraft"));
        mac.sort();
        mac.dedup();
        assert!(mac.iter().any(|e| e == "pptx") && mac.iter().any(|e| e == "deckcraft"), "Info.plist.in: {mac:?}");
        // Every document type names a content type too, so "Change All" in Finder's Get Info works.
        let names = plist.matches("<key>CFBundleTypeName</key>").count();
        let content_types = plist.matches("<key>LSItemContentTypes</key>").count();
        assert!(names > 0 && names == content_types, "{names} document types, {content_types} with LSItemContentTypes");

        let mut win: Vec<String> = wxs
            .lines()
            .filter(|l| l.contains("SupportedTypes\" Name=\"."))
            .filter_map(|l| l.split("Name=\".").nth(1)?.split('"').next().map(str::to_string))
            .collect();
        win.sort();

        let mime_ext = |m: &str| match m {
            "application/x-deckcraft" => Some("deckcraft"),
            "application/vnd.openxmlformats-officedocument.presentationml.presentation" => Some("pptx"),
            "application/vnd.openxmlformats-officedocument.presentationml.template" => Some("potx"),
            "application/vnd.openxmlformats-officedocument.presentationml.slideshow" => Some("ppsx"),
            _ => None,
        };
        let mut linux: Vec<String> = desktop
            .lines()
            .find_map(|l| l.strip_prefix("MimeType="))
            .unwrap_or_default()
            .split(';')
            .filter(|m| !m.is_empty())
            .map(|m| mime_ext(m).unwrap_or(m).to_string()) // an unknown MIME type shows up in the diff
            .collect();
        linux.sort();

        assert_eq!(mac, win, "macOS and Windows offer different file types");
        assert_eq!(mac, linux, "macOS and Linux offer different file types");
    }

    /// #1114: a shortcut that names an MSI `<Icon>` shows a blank icon once an update deletes the
    /// cached copy of it, and a taskbar pin (a copy of the shortcut) is never repaired. Shortcuts
    /// take the icon embedded in `deckcraft.exe`; only `ARPPRODUCTICON` uses the MSI `<Icon>`.
    #[test]
    fn windows_shortcuts_use_the_icon_embedded_in_the_exe() {
        let wxs = include_str!("../../../packaging/windows/deckcraft.wxs");
        let shortcuts: Vec<&str> = wxs.split("<Shortcut").skip(1).filter_map(|s| s.split("/>").next()).collect();
        assert!(!shortcuts.is_empty(), "deckcraft.wxs has no shortcut");
        for shortcut in shortcuts {
            assert!(!shortcut.contains("Icon="), "a shortcut names an MSI <Icon>:{shortcut}");
        }
        assert!(wxs.contains("<Property Id=\"ARPPRODUCTICON\""), "Apps and features keeps its icon");
    }

    /// The shortcuts take the exe's own icon, and `package.ps1 -SkipBuild` packages an existing
    /// exe without `DECKCRAFT_REQUIRE_WINRES`, so the script has to refuse one with no icon.
    #[test]
    fn the_windows_package_script_rejects_an_exe_without_an_icon() {
        let script = include_str!("../../../packaging/windows/package.ps1");
        assert!(script.contains("ExtractIconEx"), "package.ps1 does not count the exe's icons");
        assert!(script.contains("has no embedded icon"), "package.ps1 does not reject an iconless exe");
    }
}
