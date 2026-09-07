mod analysis;
mod arrangement;
mod backend;
mod chrome;
mod engine;
mod mixer;
mod model;
mod piano;
mod position;
mod scopes;
#[cfg(test)]
mod tests;
mod theme;
#[cfg(test)]
mod theme_tests;
mod ui;
mod window_chrome;
mod wire;

use gpui::{prelude::*, *};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let mut args = std::env::args().skip(1);
    let mut socket = None;
    let mut headless = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--socket" => socket = args.next().map(std::path::PathBuf::from),
            "--headless" => headless = true,
            "--help" => {
                println!("oxitone-preview --socket <private unix socket> [--headless]");
                return Ok(());
            }
            _ => return Err(format!("unknown argument: {arg}").into()),
        }
    }
    let backend = backend::Backend::start(
        socket.ok_or("--socket is required; launch with oxitone preview <entry.ts>")?,
        headless,
    )?;
    if headless {
        while !backend.stopped() {
            match backend
                .events
                .recv_timeout(std::time::Duration::from_millis(100))
            {
                Ok(model::UiEvent::Shutdown) => break,
                Ok(_) | Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(_) => break,
            }
        }
        return Ok(());
    }
    Application::new().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1440.), px(920.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Oxitone · Project Preview".into()),
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(18.), px(21.))),
                }),
                window_min_size: Some(size(px(1060.), px(720.))),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| ui::Preview::new(backend, window, cx)),
        )
        .expect("open preview window");
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        cx.activate(true);
    });
    Ok(())
}
