use super::{lock_recover, HlsBuffer};
use lofty::{prelude::*, probe::Probe};
use std::fs;
use std::io::Cursor;
use std::path::Path;

impl HlsBuffer {
    pub fn save_to_folder<P: AsRef<Path>>(&self, folder_path: P) -> std::io::Result<()> {
        let inner = lock_recover(&self.inner);

        fs::create_dir_all(&folder_path)?;

        for (index, segment_opt) in inner.segments.iter().enumerate() {
            if let Some(segment) = segment_opt {
                let file_path = if index == 0 && inner.has_init_segment {
                    folder_path.as_ref().join("init.m4a")
                } else {
                    let segment_index = if inner.has_init_segment {
                        index - 1
                    } else {
                        index
                    };
                    folder_path
                        .as_ref()
                        .join(format!("segment{}.m4a", segment_index))
                };

                fs::write(file_path, &segment.data)?;
            }
        }

        Ok(())
    }

    pub fn load_from_folder<P: AsRef<Path>>(&self, folder_path: P) -> std::io::Result<()> {
        let folder = folder_path.as_ref();

        let init_path = folder.join("init.m4a");
        let init_segment = if init_path.exists() {
            Some(fs::read(init_path.clone())?)
        } else {
            None
        };

        let mut segments_data: Vec<Vec<u8>> = Vec::new();
        let mut durations: Vec<f64> = Vec::new();
        let mut segment_index = 0;

        loop {
            let segment_path = folder.join(format!("segment{}.m4a", segment_index));
            if segment_path.exists() {
                let data = fs::read(&segment_path)?;

                let actual_data = match &init_segment {
                    Some(init) => [init.as_slice(), data.as_slice()].concat(),
                    None => data.clone(),
                };

                if let Some(duration) = Self::extract_duration(&actual_data) {
                    if duration > 0.0 {
                        durations.push(duration);
                    } else {
                        durations.push(10.0);
                    }
                } else {
                    durations.push(10.0);
                }

                segments_data.push(data);
                segment_index += 1;
            } else {
                break;
            }
        }

        if segments_data.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No segment files found in folder",
            ));
        }

        let bitrate = {
            let mut combined_data = Vec::new();
            if let Some(ref init_data) = init_segment {
                combined_data.extend_from_slice(init_data);
            }
            if let Some(first_segment) = segments_data.first() {
                combined_data.extend_from_slice(first_segment);
            }

            if !combined_data.is_empty() {
                Self::extract_bitrate(&combined_data).unwrap_or(0)
            } else {
                0
            }
        };

        let segment_count = segments_data.len();
        self.init(segment_count, durations, init_segment);

        for (idx, data) in segments_data.into_iter().enumerate() {
            self.write_chunk(idx as u64, data);
        }

        if bitrate > 0 {
            self.set_bitrate(bitrate * 1000);
        }

        Ok(())
    }

    fn extract_bitrate(data: &[u8]) -> Option<u64> {
        let cursor = Cursor::new(data);
        let probe = Probe::new(cursor).guess_file_type().ok()?.read().ok()?;

        let properties = probe.properties();
        Some(properties.audio_bitrate()? as u64)
    }

    fn extract_duration(data: &[u8]) -> Option<f64> {
        let cursor = Cursor::new(data);
        let probe = Probe::new(cursor).guess_file_type().ok()?.read().ok()?;

        let properties = probe.properties();
        let duration_millis = properties.duration().as_millis() as f64;
        Some(duration_millis / 1000.0)
    }
}
