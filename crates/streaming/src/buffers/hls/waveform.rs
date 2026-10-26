use super::{lock_recover, HlsBuffer};

impl HlsBuffer {
    pub fn get_waveforms(&self) -> Vec<crate::state::types::Waveform> {
        let inner = lock_recover(&self.inner);
        inner
            .segments
            .iter()
            .filter_map(|s| s.as_ref().and_then(|seg| seg.waveform.clone()))
            .collect()
    }

    pub fn set_segment_waveform(&self, index: usize, waveform: crate::state::types::Waveform) {
        let mut inner = lock_recover(&self.inner);
        let init_offset = if inner.has_init_segment { 1 } else { 0 };
        let real_index = index + init_offset;

        if let Some(Some(segment)) = inner.segments.get_mut(real_index) {
            segment.waveform = Some(waveform);
        }
    }
}
