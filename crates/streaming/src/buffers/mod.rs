pub mod chunked;
pub mod hls;
pub mod sync;

#[cfg(test)]
mod poison_recovery_tests;

pub trait Buffer: Send + Sync + std::fmt::Debug {
    fn is_complete(&self) -> bool;
    fn get_all_chunks(&self) -> Vec<(u64, Vec<u8>)>;
    fn has_offset(&self, offset: u64) -> bool;
    fn total_size(&self) -> Option<u64>;
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Option<usize>;
    fn progress(&self) -> f64;
    fn waveform_enabled(&self) -> bool;
    fn get_waveforms(&self) -> Vec<crate::state::types::Waveform>;
}

impl Buffer for chunked::ChunkedBuffer {
    fn waveform_enabled(&self) -> bool {
        self.waveform_enabled()
    }
    fn get_waveforms(&self) -> Vec<crate::state::types::Waveform> {
        self.get_waveforms()
    }
    fn progress(&self) -> f64 {
        (*self).progress()
    }
    fn is_complete(&self) -> bool {
        (*self).is_complete()
    }
    fn get_all_chunks(&self) -> Vec<(u64, Vec<u8>)> {
        (*self).get_all_chunks()
    }
    fn has_offset(&self, offset: u64) -> bool {
        let chunk_index = (*self).chunk_index_for_offset(offset);
        (*self).has_chunk(chunk_index)
    }
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Option<usize> {
        (*self).read_at(offset, buf)
    }
    fn total_size(&self) -> Option<u64> {
        (*self).total_size()
    }
}

impl Buffer for hls::HlsBuffer {
    fn waveform_enabled(&self) -> bool {
        self.waveform_enabled()
    }
    fn get_waveforms(&self) -> Vec<crate::state::types::Waveform> {
        self.get_waveforms()
    }
    fn is_complete(&self) -> bool {
        (*self).is_complete()
    }
    fn get_all_chunks(&self) -> Vec<(u64, Vec<u8>)> {
        (*self).get_all_chunks()
    }
    fn has_offset(&self, offset: u64) -> bool {
        (*self).has_offset(offset)
    }
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Option<usize> {
        (*self).read_at(offset, buf)
    }
    fn total_size(&self) -> Option<u64> {
        (*self).total_size()
    }
    fn progress(&self) -> f64 {
        (*self).progress()
    }
}
