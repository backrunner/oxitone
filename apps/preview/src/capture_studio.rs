//! Native titlebar/rack/sidebar/About acceptance on the disposable DAW fixture.
use crate::{ui::Preview, window_manager::WindowId, window_navigation::View};
use gpui::*;

#[derive(Default)]
pub struct Smoke {
    stage: u8,
    revision: u64,
    pattern: String,
    at: Point<Pixels>,
    cursor: u64,
}
impl Smoke {
    pub fn complete(&self) -> bool {
        std::env::var_os("OXITONE_PREVIEW_CAPTURE_STUDIO").is_none() || self.stage == 40
    }
    pub fn step(&mut self, view: &Entity<Preview>, window: &mut Window, cx: &mut App) {
        if self.complete() {
            return;
        }
        match self.stage {
            0 => view.update(cx, |s, cx| {
                self.revision = s.document.view.as_ref().unwrap().revision;
                self.pattern = s.active_pattern().unwrap().id.clone();
                assert!(crate::pattern_navigation::patterns(s.project.as_ref().unwrap()).len() > 1);
                s.workspace.dock_open = false;
                s.document.show_code = false;
                s.open_view(View::Patterns, window);
                cx.notify();
            }),
            1 => {
                let bounds = view.read(cx).pattern_picker.button.get();
                assert!(bounds.left() > px(100.) && bounds.right() < window.viewport_size().width);
                self.at = point(bounds.right() + px(37.), bounds.center().y);
                pointer(window, self.at, false, cx);
            }
            2 | 4 | 6 | 9 | 15 | 17 | 20 | 23 | 28 | 31 => pointer(window, self.at, true, cx),
            3 => {
                let s = view.read(cx);
                assert_ne!(s.active_pattern().unwrap().id, self.pattern);
                assert!(s.document.patterns_open && !s.document.windows.piano_open);
                let b = s.pattern_picker.button.get();
                self.at = point(b.right() + px(13.), b.center().y);
                pointer(window, self.at, false, cx);
            }
            5 => {
                assert_eq!(view.read(cx).active_pattern().unwrap().id, self.pattern);
                self.at = view.read(cx).pattern_picker.button.get().center();
                pointer(window, self.at, false, cx);
            }
            7 => {
                assert!(view.read(cx).pattern_picker.selected.is_some());
                press("end", window, cx);
                press("enter", window, cx);
                assert_ne!(view.read(cx).active_pattern().unwrap().id, self.pattern);
                assert!(view.read(cx).pattern_picker.selected.is_none());
                assert_eq!(
                    view.read(cx).document.view.as_ref().unwrap().revision,
                    self.revision
                );
            }
            8 => {
                self.at = view.read(cx).document.tempo.bounds.get().center();
                assert!(self.at.y < px(44.), "BPM moved into titlebar");
                pointer(window, self.at, false, cx);
            }
            10 => {
                assert!(view.read(cx).document.tempo.input.is_some());
                for key in ["1", "3", "2", "enter"] {
                    press(key, window, cx);
                }
            }
            11 => {
                let s = view.read(cx);
                if !s.document_ready() {
                    return;
                }
                assert_eq!(s.project.as_ref().unwrap().snapshot.tempo_map[0].bpm, 132.);
                assert_eq!(
                    s.document.view.as_ref().unwrap().revision,
                    self.revision + 1
                );
                press("cmd-z", window, cx);
            }
            12 => {
                let s = view.read(cx);
                if !s.document_ready() {
                    return;
                }
                assert_eq!(s.project.as_ref().unwrap().snapshot.tempo_map[0].bpm, 120.);
                assert_eq!(
                    s.document.view.as_ref().unwrap().revision,
                    self.revision + 2
                );
                press("cmd-s", window, cx);
            }
            13 => {
                if !view.read(cx).document_ready()
                    || view.read(cx).document.view.as_ref().unwrap().modified
                {
                    return;
                }
                view.update(cx, |s, cx| {
                    s.open_view(View::Browser, window);
                    cx.notify();
                });
            }
            14 => {
                let s = view.read(cx);
                let b = s.document.windows.bounds(WindowId::Browser);
                self.at = s.document.windows.desktop.get().origin
                    + point(px(b.x + b.width - 72.), px(b.y + 15.));
                pointer(window, self.at, false, cx);
            }
            16 => {
                let s = view.read(cx);
                assert!(s.workspace.browser_docked && s.document.browser_open);
                assert!(s.document.windows.desktop.get().origin.x >= px(190.));
                assert!(!s.document.windows.visible.contains(&WindowId::Browser));
                self.at = point(
                    px(s.workspace.browser_width - 50.),
                    s.document.windows.desktop.get().origin.y + px(17.),
                );
                pointer(window, self.at, false, cx);
            }
            18 => {
                let s = view.read(cx);
                assert!(!s.workspace.browser_docked);
                assert_eq!(s.document.windows.front(), Some(WindowId::Browser));
                view.update(cx, |s, cx| {
                    s.dock_browser();
                    s.open_view(View::Patterns, window);
                    cx.notify();
                });
            }
            19 | 22 => {
                if self.stage == 22 {
                    let s = view.read(cx);
                    if !s.playback.playing
                        || !s.analysis.get("mix_master").is_some_and(|n| {
                            n.peak > 0. && n.wave.iter().any(|v| v[0] != 0. || v[1] != 0.)
                        })
                    {
                        return;
                    }
                    self.cursor = s.playback.cursor;
                }
                let b = view.read(cx).workflow_bounds.get();
                self.at = point(b.right() - px(14.), b.center().y);
                pointer(window, self.at, false, cx);
            }
            21 => {
                let s = view.read(cx);
                if s.metronome_pending.is_some() {
                    return;
                }
                assert!(s.metronome);
                press("space", window, cx);
            }
            24 => {
                let s = view.read(cx);
                if s.metronome_pending.is_some() {
                    return;
                }
                assert!(!s.metronome);
                assert!(s.playback.cursor >= self.cursor);
                assert_eq!(
                    s.document.view.as_ref().unwrap().revision,
                    self.revision + 2
                );
                press("space", window, cx);
            }
            25 => window.dispatch_action(Box::new(crate::about::ShowAbout), cx),
            26 => {
                assert!(view.read(cx).show_about);
                for key in ["delete", "cmd-z", "space", "f7"] {
                    press(key, window, cx);
                }
                assert!(view.read(cx).show_about && view.read(cx).document.pending.is_none());
                assert!(!view.read(cx).is_playing());
                if std::env::var("OXITONE_PREVIEW_CAPTURE_STUDIO").is_ok_and(|v| v == "about") {
                    self.finish(view, cx);
                    return;
                }
                press("escape", window, cx);
            }
            27 => {
                assert!(!view.read(cx).show_about);
                let s = view.read(cx);
                let b = s.document.windows.bounds(WindowId::Patterns);
                self.at = s.document.windows.desktop.get().origin
                    + point(px(b.x + b.width - 31.), px(b.y + 108.));
                pointer(window, self.at, false, cx);
            }
            29 => view.update(cx, |s, cx| {
                assert_eq!(s.document.windows.front(), Some(WindowId::Piano));
                assert!(s.piano.channel.is_some());
                assert_eq!(
                    s.piano_pattern().unwrap().id,
                    s.active_pattern().unwrap().id
                );
                s.close_internal(WindowId::Piano, window);
                cx.notify();
            }),
            30 => {
                self.at.x -= px(40.);
                pointer(window, self.at, false, cx);
            }
            32 => view.update(cx, |s, cx| {
                let Some(id @ WindowId::Plugin(_)) = s.document.windows.front() else {
                    panic!("rack instrument opens its native plugin panel");
                };
                s.close_internal(id, window);
                // Keep the short fixture's note onset in view for the live waveform capture.
                s.loop_enabled = true;
                s.loop_start = 0.;
                s.loop_end = 1.;
                s.seek(0.);
                s.play();
                cx.notify();
            }),
            33 => {
                let s = view.read(cx);
                if !s.playback.playing
                    || !s.analysis.get("mix_master").is_some_and(|n| {
                        n.wave
                            .iter()
                            .any(|v| v[0].abs() > 0.05 || v[1].abs() > 0.05)
                    })
                {
                    return;
                }
            }
            34 => {
                self.finish(view, cx);
                return;
            }
            _ => unreachable!(),
        }
        self.stage += 1;
    }
    fn finish(&mut self, view: &Entity<Preview>, cx: &App) {
        let s = view.read(cx);
        assert_eq!(
            s.document.view.as_ref().unwrap().revision,
            self.revision + 2
        );
        assert!(!s.workspace.dock_open && s.document.patterns_open && s.workspace.browser_docked);
        assert_eq!(cx.windows().len(), 1);
        if let Some(n) = s.analysis.get("mix_master") {
            eprintln!(
                "Studio Master: playing={} samples={} peak={} rms={} waveformPeak={}",
                s.playback.playing,
                n.wave.len(),
                n.peak,
                n.rms,
                n.wave
                    .iter()
                    .flatten()
                    .fold(0_f32, |peak, value| peak.max(value.abs()))
            );
        }
        eprintln!("Studio chrome smoke passed: pattern selection, independent rack, Browser dock/float, titlebar tempo Undo/Save, live Master telemetry, runtime metronome, native About menu and modal isolation");
        self.stage = 40;
    }
}
fn press(key: &str, window: &mut Window, cx: &mut App) {
    window.dispatch_keystroke(Keystroke::parse(key).unwrap(), cx);
}
fn pointer(window: &Window, position: Point<Pixels>, up: bool, cx: &App) {
    crate::capture_pointer::dispatch(
        window,
        if up {
            PlatformInput::MouseUp(MouseUpEvent {
                position,
                ..Default::default()
            })
        } else {
            PlatformInput::MouseDown(MouseDownEvent {
                position,
                ..Default::default()
            })
        },
        cx,
    );
}
