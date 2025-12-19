use anyhow::Result;
use gpui::*;

mod cache;
mod config;
mod database;
mod downloader;
mod downloader_queue;
mod notifications;
mod platforms;
mod scanner;
mod ui;

use ui::{NDownloaderApp, actions::*};

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .init();

    Application::new().run(|cx: &mut App| {
        cx.activate(true);
        cx.on_action(quit);

        // Bind keyboard shortcuts
        cx.bind_keys([
            KeyBinding::new("cmd-q", Quit, None),
            KeyBinding::new("ctrl-q", Quit, None),
            KeyBinding::new("escape", GoBack, None),
            KeyBinding::new("cmd-w", GoBack, None),
            KeyBinding::new("ctrl-w", GoBack, None),
        ]);

        let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);

        let _window = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("NDownloader".into()),
                    appears_transparent: false,
                    traffic_light_position: None,
                }),
                window_background: WindowBackgroundAppearance::Opaque,
                focus: true,
                show: true,
                kind: WindowKind::Normal,
                is_movable: true,
                display_id: None,
                window_min_size: Some(gpui::Size {
                    width: px(800.),
                    height: px(600.),
                }),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| NDownloaderApp::new(window, cx)),
        );
    });

    Ok(())
}

fn quit(_: &Quit, cx: &mut App) {
    cx.quit();
}
