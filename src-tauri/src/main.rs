//! Runs the Monitor Lizard server in-process and loads it in a Tauri webview, with a menu-bar tray
//! item. See ADR 0004 for why this is a second binary over the loopback server and page, rather
//! than a native rewrite: one UI, not a fork of it.
use monitor_lizard::{paths, server, snapshot};
use std::{
    sync::Arc,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    utils::{
        config::{Color, WindowEffectsConfig},
        WindowEffect, WindowEffectState,
    },
    image::Image, ActivationPolicy, Manager, Theme, TitleBarStyle, WebviewUrl, WebviewWindowBuilder,
};

fn free_gib() -> f64 {
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    let p = std::ffi::CString::new("/").unwrap();
    if unsafe { libc::statvfs(p.as_ptr(), &mut s) } != 0 {
        return -1.0;
    }
    (s.f_bavail as f64 * s.f_frsize as f64) / (1u64 << 30) as f64
}

fn tray_text() -> (String, String) {
    let t = snapshot::take().totals;
    let title = format!("{:.0} GB · {}", free_gib(), t.findings);
    let tip = format!("{:.1} GiB free, {} findings, {} processes", free_gib(), t.findings, t.processes);
    (title, tip)
}

/// Where the tray's own error state points to when the server fails to start: a plain text log
/// next to the rest of this app's state, not stdout, since nothing reads a menu-bar app's terminal.
fn shell_log_path() -> std::path::PathBuf {
    paths::data_dir().join("shell.log")
}

fn log_error(msg: &str) {
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let _ = std::fs::create_dir_all(paths::data_dir());
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(shell_log_path())
        .and_then(|mut f| std::io::Write::write_all(&mut f, format!("[{ts}] {msg}\n").as_bytes()));
}

/// A frosted, translucent window instead of an opaque one: this is a small utility dashboard meant
/// to sit over the desktop, not a document window, so the macOS "Sidebar" material (the same look
/// Finder's sidebar and other panel-like windows use) reads as at-home with the rest of the system.
fn window_effects() -> WindowEffectsConfig {
    WindowEffectsConfig { effects: vec![WindowEffect::Sidebar], state: Some(WindowEffectState::Active), ..Default::default() }
}

/// `web/app.css`'s own `--bg` token, both themes: the title bar is drawn by macOS itself (not
/// the page), so matching it against the system default grey -- rather than this page's actual
/// background -- is exactly the seam this function exists to avoid.
fn page_bg_color(theme: Theme) -> Color {
    match theme {
        Theme::Light => Color(0xf7, 0xf5, 0xfb, 0xff),
        _ => Color(0x12, 0x0d, 0x1d, 0xff),
    }
}

/// Shared by a second launch and the tray's "Open dashboard": both just want the one window back.
fn focus_main_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // A second launch focuses the existing window instead of opening a second one; the
            // server and tray are already running in this process, so there is nothing else to start.
            focus_main_window(app);
        }))
        .setup(|app| {
            app.set_activation_policy(ActivationPolicy::Accessory);

            let bound = server::bind_free();
            let open = MenuItem::with_id(app, "open", "Open dashboard", bound.is_ok(), None::<&str>)?;
            let rescan = MenuItem::with_id(app, "rescan", "Rescan", bound.is_ok(), None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &rescan, &quit])?;

            let running_server = match bound {
                Ok((server, port)) => {
                    let srv = Arc::clone(&server);
                    thread::spawn(move || server::serve_on(srv));
                    let url: tauri::Url = format!("http://127.0.0.1:{port}/").parse().unwrap();
                    let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
                        .title("Monitor Lizard")
                        .inner_size(1200.0, 800.0)
                        .transparent(true)
                        .effects(window_effects())
                        // A transparent title bar paints the window's own background color instead
                        // of the system's default grey, so it stops reading as a seam above the
                        // page's own (much darker) background. No drag region or page changes
                        // needed: the traffic lights stay in their normal title-bar position.
                        .title_bar_style(TitleBarStyle::Transparent)
                        .build()?;
                    // Set once at creation and kept in sync on ThemeChanged below: the title bar
                    // itself is native chrome, not page content, so it cannot just read the page's
                    // CSS variable the way everything else does -- this is as close as a window
                    // can get to "follow the page's --bg", even though a theme forced on the page
                    // itself (not just the OS) is invisible to this process without an invoke call,
                    // which the no-commands-for-writes design (see module doc) deliberately avoids.
                    let _ = window.set_background_color(Some(page_bg_color(window.theme().unwrap_or(Theme::Dark))));
                    // The red close button destroys a window by default; without this, closing it
                    // once would make "main" gone for good, so a later "Open dashboard" click finds
                    // nothing and silently does nothing (the tray and server would keep running, but
                    // the one thing the menu item promises -- opening the dashboard -- would not happen).
                    let to_hide = window.clone();
                    window.on_window_event(move |event| match event {
                        tauri::WindowEvent::CloseRequested { api, .. } => {
                            api.prevent_close();
                            let _ = to_hide.hide();
                        }
                        tauri::WindowEvent::ThemeChanged(theme) => {
                            let _ = to_hide.set_background_color(Some(page_bg_color(*theme)));
                        }
                        _ => {}
                    });
                    Some(server)
                }
                Err(e) => {
                    log_error(&format!("server failed to start: {e}"));
                    None
                }
            };

            let started = running_server.is_some();
            // A black-and-clear template image: macOS recolours it for a light or dark menu bar.
            let mut tray = TrayIconBuilder::with_id("main").icon_as_template(true);
            match Image::from_bytes(include_bytes!("../icons/tray.png")) {
                Ok(icon) => tray = tray.icon(icon),
                Err(e) => log_error(&format!("tray icon unreadable, showing text only: {e}")),
            }
            let tray = tray
                .title(if started { "…".to_string() } else { "Monitor Lizard: error".to_string() })
                .tooltip(if started { String::new() } else { format!("Server failed to start; see {}", shell_log_path().display()) })
                .menu(&menu)
                .on_menu_event(move |app, ev| match ev.id().as_ref() {
                    "open" => focus_main_window(app),
                    "rescan" => {
                        let (t, tip) = tray_text();
                        if let Some(tr) = app.tray_by_id("main") {
                            let _ = tr.set_title(Some(t));
                            let _ = tr.set_tooltip(Some(tip));
                        }
                    }
                    "quit" => {
                        if let Some(srv) = &running_server {
                            srv.unblock();
                        }
                        app.exit(0);
                    }
                    _ => {}
                })
                .build(app)?;

            if started {
                let handle = app.handle().clone();
                thread::spawn(move || loop {
                    let (t, tip) = tray_text();
                    let tr = tray.clone();
                    let _ = handle.run_on_main_thread(move || {
                        let _ = tr.set_title(Some(t));
                        let _ = tr.set_tooltip(Some(tip));
                    });
                    thread::sleep(Duration::from_secs(10));
                });
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("tauri run");
}
