mod analysis;
mod arrangement;
mod automation_curve;
mod automation_edit;
mod automation_editor;
mod automation_labels;
#[cfg(test)]
mod automation_labels_tests;
mod automation_lanes;
mod automation_paint;
mod automation_panel;
mod automation_state;
mod automation_thumbnail;
mod backend;
mod capture;
mod capture_builtin;
mod capture_controls;
mod capture_daw;
mod capture_editing;
mod capture_library;
mod capture_mixer;
mod capture_navigation;
mod capture_piano;
mod capture_plugin_install;
mod capture_pointer;
#[cfg(target_os = "macos")]
mod capture_surface;
mod capture_synth;
mod capture_track_drag;
mod capture_transport;
mod capture_ui_review;
mod capture_windows;
mod capture_workspace;
mod channel_header;
mod chrome;
mod close_state;
#[cfg(test)]
mod close_state_tests;
mod document_controls;
mod document_review;
#[cfg(test)]
mod document_tests;
mod document_ui;
mod document_wire;
#[cfg(test)]
mod drag_projection_tests;
#[cfg(test)]
mod editing_bench;
mod engine;
mod internal_window_frame;
mod internal_windows;
mod mixer;
mod mixer_actions;
mod mixer_chain;
mod mixer_control;
mod mixer_edit;
mod mixer_flow;
mod mixer_inspector;
mod mixer_meter;
mod mixer_model;
mod mixer_routes;
#[cfg(test)]
mod mixer_routes_tests;
mod mixer_routing_view;
mod mixer_strip;
mod model;
mod note_brush;
mod note_commit;
mod note_edit;
mod note_gesture;
mod note_selection;
mod note_transform;
mod note_velocity;
#[cfg(test)]
mod note_velocity_tests;
mod parameter_format;
mod parameter_view;
mod pattern_manager;
mod pattern_parts;
mod pattern_preview;
mod piano;
mod piano_actions;
mod piano_divider;
mod piano_feedback;
mod piano_layout;
#[cfg(test)]
mod piano_layout_tests;
mod piano_note_paint;
mod piano_paint;
mod piano_snap;
mod piano_state;
mod piano_toolbar;
mod playback_controls;
#[cfg(test)]
mod playback_tests;
mod playlist_actions;
mod playlist_clips;
mod playlist_edit;
mod playlist_lane;
mod playlist_projection;
mod plugin_builtin_controls;
mod plugin_capture;
mod plugin_catalog;
mod plugin_choice;
mod plugin_color_plot;
mod plugin_control_input;
mod plugin_controls;
mod plugin_details;
#[cfg(test)]
mod plugin_details_tests;
mod plugin_dial;
mod plugin_dynamics_plot;
mod plugin_edit;
mod plugin_effect_layout;
mod plugin_filter_plot;
mod plugin_group_names;
mod plugin_host_controls;
mod plugin_identity;
mod plugin_layout;
mod plugin_layout_builtin;
mod plugin_layout_registry;
#[cfg(test)]
mod plugin_layout_tests;
mod plugin_layout_validation;
mod plugin_library;
mod plugin_library_input;
mod plugin_manager;
mod plugin_manager_detail;
mod plugin_manager_info;
mod plugin_manager_model;
mod plugin_open;
mod plugin_panel;
mod plugin_panel_groups;
mod plugin_panel_navigation;
mod plugin_parameters;
mod plugin_picker;
mod plugin_plot;
#[cfg(test)]
mod plugin_plot_tests;
mod plugin_plot_view;
mod plugin_resources;
mod plugin_response_view;
mod plugin_sample_plot;
mod plugin_scroll;
mod plugin_source_navigation;
mod plugin_synth_layout;
mod plugin_synth_visibility;
mod plugin_time_plot;
mod plugin_usage_list;
mod plugin_visuals;
mod plugin_watch_capture;
mod plugin_wave_view;
mod plugin_window;
mod plugin_window_input;
mod plugin_window_view;
mod pointer_capture;
mod preview_source;
mod project_edit;
mod scopes;
mod scrollbar;
mod shortcut_help;
mod shortcuts;
mod status_bar;
mod tempo_edit;
#[cfg(test)]
mod tests;
mod theme;
#[cfg(test)]
mod theme_tests;
mod timeline_input;
mod ui;
mod ui_controls;
mod ui_icons;
mod window_chrome;
mod window_close;
mod window_drag;
mod window_manager;
#[cfg(test)]
mod window_manager_tests;
mod window_navigation;
mod wire;
mod workspace;
mod workspace_layout;
mod workspace_panels;
mod workspace_resize;
mod workspace_view;

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
        headless
            || std::env::var_os("OXITONE_PREVIEW_CAPTURE").is_some()
            || std::env::var("OXITONE_PREVIEW_SIMULATED").is_ok_and(|value| value == "1"),
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
        assert!(
            !cx.text_system().all_font_names().is_empty(),
            "Preview requires GPUI's native font-kit backend"
        );
        let bounds = Bounds::centered(None, capture::window_size(), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Oxitone · Project Preview".into()),
                    appears_transparent: true,
                    // GPUI expects the traffic-light origin (not its center).
                    // 44px chrome keeps the 20px controls vertically centered.
                    traffic_light_position: Some(point(px(18.), px(17.))),
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
mod capture_configuration;
mod configuration_edit;
mod configuration_editor;
mod configuration_panel;
mod configuration_wire;
mod effect_order;
mod rack_review;
