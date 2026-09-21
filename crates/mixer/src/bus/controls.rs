//! Compilation-time native capability discovery. Never used by the audio callback.
impl super::MixerEngine {
    pub fn native_insert_control(
        &self,
        bus: &str,
        insert: usize,
    ) -> Option<std::sync::Arc<dyn oxitone_graph::control::NativeControl>> {
        self.buses
            .get(*self.index.get(bus)?)?
            .inserts
            .get(insert)?
            .instance
            .native_control()
    }
}
