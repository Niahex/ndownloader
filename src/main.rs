use anyhow::Result;
use gpui::*;

mod config;
mod database;
mod downloader;
mod platforms;
mod ui;

use ui::NDownloadApp;

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    Application::new().run(|cx: &mut App| {
        cx.activate(true);
        cx.on_action(quit);

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
            |_window, cx| cx.new(|cx| NDownloadApp::new(cx)),
        );
    });

    Ok(())
}

fn quit(_: &Quit, cx: &mut App) {
    cx.quit();
}

actions!(ndownload, [Quit]);
