//! Single-component fixture: a mode switch changes buses, latency and the parameter table.
use super::*;
use std::cell::RefCell;
use vst3::{ComPtr, ComRef};

mod audio;
mod controller;
mod processor;
pub(super) const CID: TUID = uid(0x6E332252, 0x54224A00, 0xAA69301A, 0xF3187981);
pub(super) const DENSE_CID: TUID = uid(0x6E332252, 0x54224A00, 0xAA69301A, 0xF3187982);
pub(super) const LIVE_CID: TUID = uid(0x6E332252, 0x54224A00, 0xAA69301A, 0xF3187984);
pub(super) const GRAPH_CID: TUID = uid(0x6E332252, 0x54224A00, 0xAA69301A, 0xF3187985);
pub(super) struct Dynamic {
    dense: bool,
    live: bool,
    graph: bool,
    delay: RefCell<[[f32; 64]; 2]>,
    cursor: Cell<usize>,
    mode: Cell<u8>,
    gain: Cell<f64>,
    dsp_gain: Cell<f64>,
    bulk_requested: Cell<bool>,
    active: Cell<bool>,
    processing: Cell<bool>,
    configured: Cell<bool>,
    deactivations: Cell<u32>,
    handler: RefCell<Option<ComPtr<IComponentHandler>>>,
    buses: [buses::Buses; 2],
}
impl Class for Dynamic {
    type Interfaces = (IComponent, IAudioProcessor, IEditController);
}
impl Dynamic {
    pub fn new() -> Self {
        Self {
            dense: false,
            live: false,
            graph: false,
            delay: RefCell::new([[0.; 64]; 2]),
            cursor: Cell::new(0),
            mode: Cell::new(0),
            gain: Cell::new(0.5),
            dsp_gain: Cell::new(0.5),
            bulk_requested: Cell::new(false),
            active: Cell::new(false),
            processing: Cell::new(false),
            configured: Cell::new(false),
            deactivations: Cell::new(0),
            handler: RefCell::new(None),
            buses: [buses::Buses::new(1), buses::Buses::new(3)],
        }
    }
    pub fn dense() -> Self {
        Self {
            dense: true,
            ..Self::new()
        }
    }
    pub fn live() -> Self {
        Self {
            live: true,
            ..Self::new()
        }
    }
    pub fn graph() -> Self {
        Self {
            graph: true,
            buses: [buses::Buses::new(1), buses::Buses::mono_main()],
            ..Self::live()
        }
    }
    fn buses(&self) -> &buses::Buses {
        &self.buses[(self.mode.get() == 1) as usize]
    }
    fn restart(&self, flags: i32) {
        if let Some(handler) = self.handler.borrow().as_ref() {
            unsafe {
                handler.restartComponent(flags);
            }
        }
    }
    fn set_mode(&self, mode: u8) {
        if self.mode.replace(mode) == mode {
            return;
        }
        self.configured.set(false);
        self.restart(
            RestartFlags_::kIoChanged
                | RestartFlags_::kLatencyChanged
                | RestartFlags_::kParamTitlesChanged,
        );
    }
    unsafe fn read_state(&self, state: *mut IBStream) -> tresult {
        let Some(state) = ComRef::from_raw(state) else {
            return kInvalidArgument;
        };
        let mut bytes = [0u8; 9];
        let mut read = 0;
        if state.read(bytes.as_mut_ptr().cast(), 9, &mut read) != kResultOk
            || read != 9
            || bytes[0] > 3
        {
            return kInvalidArgument;
        }
        let gain = f64::from_le_bytes(bytes[1..].try_into().unwrap());
        if !gain.is_finite() || !(0.0..=1.0).contains(&gain) {
            return kInvalidArgument;
        }
        self.set_mode(bytes[0]);
        self.gain.set(gain);
        self.dsp_gain.set(gain);
        kResultOk
    }
    unsafe fn write_state(&self, state: *mut IBStream) -> tresult {
        let Some(state) = ComRef::from_raw(state) else {
            return kInvalidArgument;
        };
        let mut bytes = [0u8; 9];
        bytes[0] = self.mode.get();
        bytes[1..].copy_from_slice(&self.dsp_gain.get().to_le_bytes());
        let mut written = 0;
        let result = state.write(bytes.as_mut_ptr().cast(), 9, &mut written);
        if written == 9 {
            result
        } else {
            kResultFalse
        }
    }
}
impl IPluginBaseTrait for Dynamic {
    unsafe fn initialize(&self, _context: *mut FUnknown) -> tresult {
        kResultOk
    }
    unsafe fn terminate(&self) -> tresult {
        self.handler.borrow_mut().take();
        kResultOk
    }
}
impl IComponentTrait for Dynamic {
    unsafe fn getControllerClassId(&self, _class_id: *mut TUID) -> tresult {
        kNotImplemented
    }
    unsafe fn setIoMode(&self, _mode: IoMode) -> tresult {
        kResultOk
    }
    unsafe fn getBusCount(&self, media: MediaType, dir: BusDirection) -> i32 {
        self.buses().count(media, dir)
    }
    unsafe fn getBusInfo(
        &self,
        media: MediaType,
        dir: BusDirection,
        index: i32,
        bus: *mut BusInfo,
    ) -> tresult {
        self.buses().info(media, dir, index, bus)
    }
    unsafe fn getRoutingInfo(
        &self,
        _input: *mut RoutingInfo,
        _output: *mut RoutingInfo,
    ) -> tresult {
        kNotImplemented
    }
    unsafe fn activateBus(
        &self,
        media: MediaType,
        dir: BusDirection,
        index: i32,
        state: TBool,
    ) -> tresult {
        self.buses().activate(media, dir, index, state)
    }
    unsafe fn setActive(&self, state: TBool) -> tresult {
        if state == 0 && self.active.get() {
            self.deactivations.set(self.deactivations.get() + 1);
        }
        self.active.set(state != 0);
        kResultOk
    }
    unsafe fn setState(&self, state: *mut IBStream) -> tresult {
        self.read_state(state)
    }
    unsafe fn getState(&self, state: *mut IBStream) -> tresult {
        self.write_state(state)
    }
}
