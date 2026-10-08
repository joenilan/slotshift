#![cfg_attr(windows, windows_subsystem = "windows")]
mod theme;
mod transfer_ui;
mod ui;
mod ui_view;
use gpui_kit::component::TitleBar;
use gpui_kit::*;
use slotshift::{model::Settings, storage::Store};
use std::path::PathBuf;
fn main() {
    if let Err(error) = run() {
        show_error(&format!("{error:#}"));
    }
}
fn run() -> anyhow::Result<()> {
    let mut demo = false;
    let mut data_dir = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--demo" => demo = true,
            "--data-dir" => {
                data_dir =
                    Some(PathBuf::from(args.next().ok_or_else(|| {
                        anyhow::anyhow!("--data-dir requires a folder.")
                    })?))
            }
            _ => anyhow::bail!(
                "Unknown option: {arg}. Available options: --demo, --data-dir <folder>."
            ),
        }
    }
    let temp = if demo {
        Some(tempfile::tempdir()?)
    } else {
        None
    };
    let custom = data_dir.is_some();
    let root = match &temp {
        Some(t) => t.path().join("Slotshift"),
        None => data_dir.unwrap_or(Store::default_root()?),
    };
    let dirs = directories::BaseDirs::new();
    let user_home = if demo || custom {
        None
    } else {
        dirs.as_ref().map(|d| d.home_dir())
    };
    let (store, mut settings) = Store::open(root, user_home)?;
    if demo {
        create_demo(&store, &mut settings)?;
    }
    let mut app = application().with_assets(assets::Assets);
    app = app.with_quit_mode(QuitMode::LastWindowClosed);
    app.run(move |cx| {
        init(cx);
        theme::apply(cx);
        let bounds = Bounds::centered(None, size(px(1100.), px(840.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(900.), px(660.))),
            ..TitleBar::window_options()
        };
        match open_window(options, cx, move |window, cx| {
            window.set_window_title("Slotshift");
            cx.new(|cx| ui::Launcher::new(store, settings, demo, window, cx))
        }) {
            Ok(_) => cx.activate(true),
            Err(e) => {
                show_error(&format!("Could not open the GPU-rendered window: {e}"));
                cx.quit();
            }
        }
    });
    drop(temp);
    Ok(())
}
fn create_demo(store: &Store, settings: &mut Settings) -> anyhow::Result<()> {
    for name in ["Personal", "Workbench", "Side projects"] {
        store.add_account(settings, name, None)?;
    }
    settings.selected = settings.accounts.first().map(|a| a.id.clone());
    if let Some(a) = settings.current_mut() {
        a.project = r"E:\Projects\atlas".into();
        a.options.yolo = true;
        a.options.live_search = true;
    }
    settings.recent_projects = vec![
        r"E:\Projects\atlas".into(),
        r"E:\Projects\notes".into(),
        r"E:\Projects\website".into(),
    ];
    store.save(settings)?;
    Ok(())
}
fn show_error(text: &str) {
    #[cfg(windows)]
    {
        #[link(name = "user32")]
        unsafe extern "system" {
            fn MessageBoxW(
                hwnd: *mut std::ffi::c_void,
                text: *const u16,
                caption: *const u16,
                kind: u32,
            ) -> i32;
        }
        let text: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
        let title: Vec<u16> = "Slotshift".encode_utf16().chain(Some(0)).collect();
        unsafe {
            MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), 0x10);
        }
    }
    #[cfg(not(windows))]
    eprintln!("Slotshift: {text}");
}
