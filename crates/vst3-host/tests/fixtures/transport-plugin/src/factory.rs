use super::*;
struct Factory {}

impl Class for Factory {
    type Interfaces = (IPluginFactory,);
}

impl IPluginFactoryTrait for Factory {
    unsafe fn getFactoryInfo(&self, info: *mut PFactoryInfo) -> tresult {
        let info = &mut *info;

        copy_cstring("Vendor", &mut info.vendor);
        copy_cstring("https://example.com", &mut info.url);
        copy_cstring("someone@example.com", &mut info.email);
        info.flags = PFactoryInfo_::FactoryFlags_::kUnicode as int32;

        kResultOk
    }

    unsafe fn countClasses(&self) -> i32 {
        21
    }

    unsafe fn getClassInfo(&self, index: i32, info: *mut PClassInfo) -> tresult {
        match index {
            0 => {
                let info = &mut *info;
                info.cid = GainProcessor::CID;
                info.cardinality = PClassInfo_::ClassCardinality_::kManyInstances as int32;
                copy_cstring("Audio Module Class", &mut info.category);
                copy_cstring(PLUGIN_NAME, &mut info.name);

                kResultOk
            }
            1 => {
                let info = &mut *info;
                info.cid = GainController::CID;
                info.cardinality = PClassInfo_::ClassCardinality_::kManyInstances as int32;
                copy_cstring("Component Controller Class", &mut info.category);
                copy_cstring(PLUGIN_NAME, &mut info.name);

                kResultOk
            }
            2..=9 => {
                let info = &mut *info;
                info.cid = if index == 2 {
                    buses::SIDECHAIN_CID
                } else if index == 3 {
                    buses::MULTIBUS_CID
                } else if index == 4 {
                    instrument::CID
                } else if index == 5 {
                    dynamic::CID
                } else if index == 6 {
                    dynamic::DENSE_CID
                } else if index == 7 {
                    GainProcessor::RECORDING_CID
                } else if index == 8 {
                    dynamic::LIVE_CID
                } else {
                    dynamic::GRAPH_CID
                };
                info.cardinality = PClassInfo_::ClassCardinality_::kManyInstances as int32;
                copy_cstring("Audio Module Class", &mut info.category);
                copy_cstring(
                    if index == 2 {
                        "Oxitone Sidechain Fixture"
                    } else if index == 3 {
                        "Oxitone Multibus Fixture"
                    } else if index == 4 {
                        "Oxitone Multioutput Instrument"
                    } else if index == 5 {
                        "Oxitone Dynamic Configuration"
                    } else if index == 6 {
                        "Oxitone Dense Configuration"
                    } else if index == 7 {
                        "Oxitone Recording Gain"
                    } else if index == 8 {
                        "Oxitone Live Restart"
                    } else {
                        "Oxitone Graph Restart"
                    },
                    &mut info.name,
                );
                kResultOk
            }
            10..=15 => {
                let info = &mut *info;
                info.cid = [
                    midi::CID,
                    midi::FX_CID,
                    midi::OVERFLOW_CID,
                    midi::INVALID_CID,
                    midi::ONLY_CID,
                    midi::GENERATOR_CID,
                ][index as usize - 10];
                info.cardinality = PClassInfo_::ClassCardinality_::kManyInstances as int32;
                copy_cstring("Audio Module Class", &mut info.category);
                copy_cstring("Oxitone MIDI Output", &mut info.name);
                kResultOk
            }
            16..=20 => {
                let info = &mut *info;
                info.cid = sysex::CLASSES[index as usize - 16];
                info.cardinality = PClassInfo_::ClassCardinality_::kManyInstances as int32;
                copy_cstring("Audio Module Class", &mut info.category);
                copy_cstring("Oxitone SysEx", &mut info.name);
                kResultOk
            }
            _ => kInvalidArgument,
        }
    }

    unsafe fn createInstance(
        &self,
        cid: FIDString,
        iid: FIDString,
        obj: *mut *mut c_void,
    ) -> tresult {
        let instance = match *(cid as *const TUID) {
            id if sysex::CLASSES.contains(&id) => {
                let index = sysex::CLASSES.iter().position(|c| *c == id).unwrap();
                let processor = if index == 4 {
                    GainProcessor::sysex_receiver()
                } else {
                    GainProcessor::midi(7 + index as u8)
                };
                Some(ComWrapper::new(processor).to_com_ptr::<FUnknown>().unwrap())
            }
            midi::CID
            | midi::FX_CID
            | midi::OVERFLOW_CID
            | midi::INVALID_CID
            | midi::ONLY_CID
            | midi::GENERATOR_CID => {
                let id = *(cid as *const TUID);
                let mode = if id == midi::CID {
                    1
                } else if id == midi::FX_CID {
                    2
                } else if id == midi::OVERFLOW_CID {
                    3
                } else if id == midi::INVALID_CID {
                    4
                } else if id == midi::ONLY_CID {
                    5
                } else {
                    6
                };
                Some(
                    ComWrapper::new(GainProcessor::midi(mode))
                        .to_com_ptr::<FUnknown>()
                        .unwrap(),
                )
            }
            dynamic::GRAPH_CID => Some(
                ComWrapper::new(dynamic::Dynamic::graph())
                    .to_com_ptr::<FUnknown>()
                    .unwrap(),
            ),
            dynamic::LIVE_CID => Some(
                ComWrapper::new(dynamic::Dynamic::live())
                    .to_com_ptr::<FUnknown>()
                    .unwrap(),
            ),
            GainProcessor::RECORDING_CID => Some(
                ComWrapper::new(GainProcessor::recording())
                    .to_com_ptr::<FUnknown>()
                    .unwrap(),
            ),
            GainProcessor::CID => Some(
                ComWrapper::new(GainProcessor::new())
                    .to_com_ptr::<FUnknown>()
                    .unwrap(),
            ),
            GainController::CID => Some(
                ComWrapper::new(GainController::new())
                    .to_com_ptr::<FUnknown>()
                    .unwrap(),
            ),
            buses::SIDECHAIN_CID => Some(
                ComWrapper::new(GainProcessor::with_buses(1))
                    .to_com_ptr::<FUnknown>()
                    .unwrap(),
            ),
            buses::MULTIBUS_CID => Some(
                ComWrapper::new(GainProcessor::with_buses(3))
                    .to_com_ptr::<FUnknown>()
                    .unwrap(),
            ),
            instrument::CID => Some(
                ComWrapper::new(GainProcessor::instrument())
                    .to_com_ptr::<FUnknown>()
                    .unwrap(),
            ),
            dynamic::CID => Some(
                ComWrapper::new(dynamic::Dynamic::new())
                    .to_com_ptr::<FUnknown>()
                    .unwrap(),
            ),
            dynamic::DENSE_CID => Some(
                ComWrapper::new(dynamic::Dynamic::dense())
                    .to_com_ptr::<FUnknown>()
                    .unwrap(),
            ),
            _ => None,
        };

        if let Some(instance) = instance {
            let ptr = instance.as_ptr();
            ((*(*ptr).vtbl).queryInterface)(ptr, iid as *mut TUID, obj)
        } else {
            kInvalidArgument
        }
    }
}

#[cfg(target_os = "windows")]
#[no_mangle]
extern "system" fn InitDll() -> bool {
    true
}

#[cfg(target_os = "windows")]
#[no_mangle]
extern "system" fn ExitDll() -> bool {
    true
}

#[cfg(target_os = "macos")]
#[no_mangle]
extern "system" fn bundleEntry(_bundle_ref: *mut c_void) -> bool {
    true
}

#[cfg(target_os = "macos")]
#[no_mangle]
extern "system" fn bundleExit() -> bool {
    true
}

#[cfg(target_os = "linux")]
#[no_mangle]
extern "system" fn ModuleEntry(_library_handle: *mut c_void) -> bool {
    true
}

#[cfg(target_os = "linux")]
#[no_mangle]
extern "system" fn ModuleExit() -> bool {
    true
}

#[no_mangle]
extern "system" fn GetPluginFactory() -> *mut IPluginFactory {
    ComWrapper::new(Factory {})
        .to_com_ptr::<IPluginFactory>()
        .unwrap()
        .into_raw()
}
