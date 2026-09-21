use super::*;

impl IEditControllerTrait for Dynamic {
    unsafe fn setComponentState(&self, state: *mut IBStream) -> tresult {
        self.read_state(state)
    }
    unsafe fn setState(&self, state: *mut IBStream) -> tresult {
        self.read_state(state)
    }
    unsafe fn getState(&self, state: *mut IBStream) -> tresult {
        self.write_state(state)
    }
    unsafe fn getParameterCount(&self) -> i32 {
        if self.dense {
            return 4096;
        }
        4 + (self.mode.get() == 1) as i32
    }
    unsafe fn getParameterInfo(&self, index: i32, info: *mut ParameterInfo) -> tresult {
        let (id, name, default, readonly) = if self.dense && (0..4096).contains(&index) {
            (index as u32, "Dense parameter", 0.5, false)
        } else {
            match index {
                0 => (0, "Mode", 0., false),
                1 => (90, "Processor gain", 0.5, true),
                2 => (91, "Latency / 128", 0., true),
                3 => (92, "Deactivations / 16", 0., true),
                4 if self.mode.get() == 1 => (7, "Expanded gain", 0.5, false),
                _ => return kInvalidArgument,
            }
        };
        let info = &mut *info;
        info.id = id;
        copy_wstring(name, &mut info.title);
        copy_wstring(name, &mut info.shortTitle);
        copy_wstring("", &mut info.units);
        info.defaultNormalizedValue = default;
        info.stepCount = if id == 0 { 3 } else { 0 };
        info.unitId = 0;
        info.flags = if readonly {
            ParameterInfo_::ParameterFlags_::kIsReadOnly
        } else {
            ParameterInfo_::ParameterFlags_::kCanAutomate
        } as i32;
        kResultOk
    }
    unsafe fn getParamStringByValue(
        &self,
        _id: u32,
        value: f64,
        string: *mut String128,
    ) -> tresult {
        copy_wstring(&value.to_string(), &mut *string);
        kResultOk
    }
    unsafe fn getParamValueByString(
        &self,
        _id: u32,
        _string: *mut TChar,
        _value: *mut f64,
    ) -> tresult {
        kNotImplemented
    }
    unsafe fn normalizedParamToPlain(&self, _id: u32, value: f64) -> f64 {
        value
    }
    unsafe fn plainParamToNormalized(&self, _id: u32, value: f64) -> f64 {
        value
    }
    unsafe fn getParamNormalized(&self, id: u32) -> f64 {
        if self.dense {
            return if id == 4095 { self.gain.get() } else { 0.5 };
        }
        match id {
            0 => self.mode.get() as f64 / 3.,
            7 => self.gain.get(),
            90 => self.dsp_gain.get(),
            91 => (self.mode.get() == 1) as u8 as f64 * 0.5,
            92 => (self.deactivations.get() as f64 / 16.).min(1.),
            _ => 0.,
        }
    }
    unsafe fn setParamNormalized(&self, id: u32, value: f64) -> tresult {
        if self.dense {
            if id == 4095 {
                self.gain.set(value);
            }
            return if id < 4096 {
                kResultOk
            } else {
                kInvalidArgument
            };
        }
        match id {
            0 => self.set_mode((value * 3.).round() as u8),
            7 if self.mode.get() == 1 => {
                self.gain.set(value);
                self.bulk_requested.set(value == 0.75);
            }
            _ => return kInvalidArgument,
        }
        kResultOk
    }
    unsafe fn setComponentHandler(&self, handler: *mut IComponentHandler) -> tresult {
        *self.handler.borrow_mut() = ComRef::from_raw(handler).map(|h| h.to_com_ptr());
        kResultOk
    }
    unsafe fn createView(&self, _name: *const c_char) -> *mut IPlugView {
        ptr::null_mut()
    }
}
