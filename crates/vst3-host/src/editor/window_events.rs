//! Bounded main-thread AppKit pumping shared by silent and live editors.
use super::{EditorWindow, Mode};
use crate::{native, Result};
use objc2::{rc::autoreleasepool, DefinedClass, MainThreadMarker};
use objc2_app_kit::{NSApplication, NSEventMask};
use objc2_foundation::{NSDate, NSDefaultRunLoopMode};
use vst3_host::Plugin;

impl EditorWindow {
    pub(crate) fn poll(
        &mut self,
        plugin: &mut Plugin,
        mtm: MainThreadMarker,
        wait: f64,
    ) -> Result<Option<bool>> {
        autoreleasepool(|_| {
            let app = NSApplication::sharedApplication(mtm);
            // A bounded batch prevents continuous input from starving audio processing.
            for index in 0..32 {
                let until =
                    NSDate::dateWithTimeIntervalSinceNow(if index == 0 { wait } else { 0. });
                // SAFETY: Foundation constant is read on the main thread.
                let Some(event) = app.nextEventMatchingMask_untilDate_inMode_dequeue(
                    NSEventMask::Any,
                    Some(&until),
                    unsafe { NSDefaultRunLoopMode },
                    true,
                ) else {
                    break;
                };
                app.sendEvent(&event);
            }
            if let Some(size) = plugin.take_editor_resize_request() {
                self.layout(size)?;
            } else if plugin.editor_can_resize() {
                let size = self
                    .window
                    .contentView()
                    .ok_or_else(|| crate::invalid("editor lost its content view"))?
                    .frame()
                    .size;
                let desired = (
                    size.width.round() as i32,
                    (size.height - 44.).round() as i32,
                );
                if desired != (self.size.0.max(220), self.size.1) {
                    let size = plugin.resize_editor(desired.0, desired.1).map_err(native)?;
                    self.layout(size)?;
                }
            }
            app.updateWindows();
            Ok(match self.actions.ivars().get() {
                0 => None,
                1 => Some(self.mode == Mode::Configuration),
                _ => Some(false),
            })
        })
    }
}
