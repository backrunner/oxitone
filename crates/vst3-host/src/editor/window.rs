//! AppKit lifetime and event dispatch for one vendor configuration view.
use crate::silent_processing::SilentProcessing;
use crate::{native, Result};
use objc2::{
    define_class, msg_send,
    rc::Retained,
    runtime::{AnyObject, ProtocolObject},
    sel, DefinedClass, MainThreadMarker, MainThreadOnly,
};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSBezelStyle, NSButton,
    NSView, NSWindow, NSWindowDelegate, NSWindowStyleMask,
};
use objc2_foundation::{NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};
use std::cell::Cell;
use vst3_host::{Plugin, WindowHandle};

define_class!(
    // SAFETY: NSObject imposes no subclass requirements. Objects never leave main.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = Cell<u8>]
    struct Actions;
    unsafe impl NSObjectProtocol for Actions {}
    impl Actions {
        #[unsafe(method(apply:))]
        fn apply(&self, _sender: Option<&AnyObject>) { self.ivars().set(1); }
        #[unsafe(method(cancel:))]
        fn cancel(&self, _sender: Option<&AnyObject>) { self.ivars().set(2); }
    }
    unsafe impl NSWindowDelegate for Actions {
        #[unsafe(method(windowShouldClose:))]
        fn should_close(&self, _window: &NSWindow) -> bool {
            self.ivars().set(2);
            false // Detach the vendor view before closing its parent.
        }
    }
);

#[path = "window_events.rs"]
mod events;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    Configuration,
    Live,
}

pub(crate) fn initialize() -> Result<MainThreadMarker> {
    let mtm = MainThreadMarker::new()
        .ok_or_else(|| crate::invalid("VST3 editor requires main thread"))?;
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    app.finishLaunching();
    Ok(mtm)
}

pub(crate) struct EditorWindow {
    mode: Mode,
    window: Retained<NSWindow>,
    container: Retained<NSView>,
    actions: Retained<Actions>,
    apply: Retained<NSButton>,
    cancel: Retained<NSButton>,
    size: (i32, i32),
}
fn valid_size(size: (i32, i32)) -> Result<(i32, i32)> {
    if !(16..=4096).contains(&size.0) || !(16..=4096).contains(&size.1) {
        return Err(crate::unsupported(
            "VST3 editor dimensions must be in 16..4096",
        ));
    }
    Ok(size)
}
impl EditorWindow {
    fn new(plugin: &Plugin, mtm: MainThreadMarker, mode: Mode) -> Result<Self> {
        let size = valid_size(plugin.get_editor_size().map_err(native)?)?;
        let mut style = NSWindowStyleMask::Titled
            | NSWindowStyleMask::Closable
            | NSWindowStyleMask::Miniaturizable;
        if plugin.editor_can_resize() {
            style |= NSWindowStyleMask::Resizable;
        }
        // SAFETY: All AppKit objects are main-thread owned; automatic close-release is disabled.
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                NSRect::new(NSPoint::new(0., 0.), NSSize::new(640., 480.)),
                style,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        unsafe {
            window.setReleasedWhenClosed(false);
        }
        window.setTitle(&NSString::from_str(&format!(
            "{} — Oxitone",
            plugin.info().name
        )));
        window.setContentMinSize(NSSize::new(220., 60.));
        window.setContentMaxSize(NSSize::new(4096., 4140.));
        let allocated = Actions::alloc(mtm).set_ivars(Cell::new(0));
        // SAFETY: NSObject initialization with its documented signature.
        let actions: Retained<Actions> = unsafe { msg_send![super(allocated), init] };
        window.setDelegate(Some(ProtocolObject::from_ref(&*actions)));
        let container = NSView::initWithFrame(
            NSView::alloc(mtm),
            NSRect::new(
                NSPoint::new(0., 44.),
                NSSize::new(size.0 as f64, size.1 as f64),
            ),
        );
        let content = window
            .contentView()
            .ok_or_else(|| crate::invalid("editor window has no content view"))?;
        content.addSubview(&container);
        // SAFETY: The retained Actions target outlives both buttons and implements these selectors.
        let (apply, cancel) = unsafe {
            (
                NSButton::buttonWithTitle_target_action(
                    &NSString::from_str("Apply"),
                    Some(&actions),
                    Some(sel!(apply:)),
                    mtm,
                ),
                NSButton::buttonWithTitle_target_action(
                    &NSString::from_str("Cancel"),
                    Some(&actions),
                    Some(sel!(cancel:)),
                    mtm,
                ),
            )
        };
        for button in [&apply, &cancel] {
            button.setBezelStyle(NSBezelStyle::Push);
            content.addSubview(button);
        }
        apply.setKeyEquivalent(&NSString::from_str("\r"));
        cancel.setKeyEquivalent(&NSString::from_str("\u{1b}"));
        if mode == Mode::Live {
            apply.setHidden(true);
            cancel.setTitle(&NSString::from_str("Close"));
        }
        let mut result = Self {
            mode,
            window,
            container,
            actions,
            apply,
            cancel,
            size,
        };
        result.layout(size)?;
        Ok(result)
    }
    fn layout(&mut self, size: (i32, i32)) -> Result<()> {
        let size = valid_size(size)?;
        let width = f64::from(size.0.max(220));
        self.window
            .setContentSize(NSSize::new(width, f64::from(size.1) + 44.));
        self.container.setFrame(NSRect::new(
            NSPoint::new(0., 44.),
            NSSize::new(size.0 as f64, size.1 as f64),
        ));
        self.apply.setFrame(NSRect::new(
            NSPoint::new(width - 100., 8.),
            NSSize::new(88., 28.),
        ));
        self.cancel.setFrame(NSRect::new(
            NSPoint::new(
                width - if self.mode == Mode::Live { 100. } else { 196. },
                8.,
            ),
            NSSize::new(88., 28.),
        ));
        self.size = size;
        Ok(())
    }
    pub(crate) fn open(plugin: &mut Plugin, mtm: MainThreadMarker, mode: Mode) -> Result<Self> {
        if !plugin.has_editor() {
            return Err(crate::unsupported("This VST3 plugin has no native editor"));
        }
        let mut ui = Self::new(plugin, mtm, mode)?;
        // SAFETY: Container is retained until close() detaches the vendor view, including errors.
        let parent = unsafe {
            WindowHandle::from_nsview(Retained::as_ptr(&ui.container) as *mut std::ffi::c_void)
        };
        if let Err(error) = plugin.open_editor(parent).map_err(native) {
            let _ = ui.close(plugin);
            return Err(error);
        }
        ui.window.center();
        ui.window.makeKeyAndOrderFront(None);
        #[allow(deprecated)]
        NSApplication::sharedApplication(mtm).activateIgnoringOtherApps(true);
        Ok(ui)
    }
    pub(crate) fn close(&mut self, plugin: &mut Plugin) -> Result<()> {
        let result = plugin.close_editor().map_err(native);
        self.window.setDelegate(None);
        self.window.close();
        result
    }
}

pub(super) fn show(
    plugin: &mut Plugin,
    silent: &mut SilentProcessing,
    mtm: MainThreadMarker,
) -> Result<bool> {
    let mut ui = EditorWindow::open(plugin, mtm, Mode::Configuration)?;
    let result = (|| -> Result<bool> {
        loop {
            let outcome = ui.poll(plugin, mtm, 1. / 60.)?;
            silent.flush(plugin)?;
            if let Some(accepted) = outcome {
                return Ok(accepted);
            }
        }
    })();
    let closed = ui.close(plugin);
    let accepted = result?;
    closed?;
    if accepted {
        silent.flush(plugin)?;
    }
    Ok(accepted)
}
