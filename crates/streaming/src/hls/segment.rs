#[derive(Debug, Clone)]
pub struct HlsSegment {
    pub url: String,
    pub duration: f64,
    pub byte_offset: u64,
    pub byte_size: Option<u64>,
    pub sequence: u64,
    pub range_length: Option<u64>,
    pub range_offset: Option<u64>,
}
#[derive(Debug, Clone)]
pub enum HlsPlaylistType {
    Master(MasterPlaylist),
    Media(MediaPlaylist),
}

#[derive(Debug, Clone)]
pub struct MasterPlaylist {
    pub variants: Vec<StreamVariant>,
}

#[derive(Debug, Clone)]
pub struct StreamVariant {
    pub uri: String,
    pub bandwidth: u64,
    pub codecs: Option<String>,
    pub resolution: Option<String>,
}

#[derive(Debug, Clone)]
pub struct InitSegment {
    pub url: String,
    pub range_length: Option<u64>,
    pub range_offset: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct MediaPlaylist {
    pub init_segment: Option<InitSegment>,
    pub segments: Vec<HlsSegment>,
    pub playlist_type: PlaylistType,
    pub target_duration: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaylistType {
    Vod,
    Live,
}
pub type HlsPlaylist = MediaPlaylist;

impl Default for HlsPlaylist {
    fn default() -> Self {
        Self::new()
    }
}

impl HlsPlaylist {
    pub fn new() -> Self {
        Self {
            init_segment: None,
            segments: Vec::new(),
            playlist_type: PlaylistType::Vod,
            target_duration: 10,
        }
    }

    pub fn total_duration(&self) -> f64 {
        self.segments.iter().map(|s| s.duration).sum()
    }

    pub fn estimated_size(&self) -> Option<u64> {
        let sizes: Option<Vec<u64>> = self.segments.iter().map(|s| s.byte_size).collect();

        sizes.map(|s| s.iter().sum())
    }

    pub fn find_segment_by_time(&self, time_seconds: f64) -> Option<usize> {
        let mut current_time = 0.0;

        for (idx, segment) in self.segments.iter().enumerate() {
            current_time += segment.duration;

            if current_time >= time_seconds {
                return Some(idx);
            }
        }

        None
    }

    pub fn detect_format(&self) -> Option<String> {
        if let Some(segment) = self.segments.first() {
            let url = segment.url.to_lowercase();
            if url.contains("opus") {
                return Some("opus".to_string());
            }
            if url.contains("ogg") {
                return Some("ogg".to_string());
            }
        }
        None
    }
}

impl MasterPlaylist {
    pub fn select_variant(&self, available_bandwidth_bps: u64) -> &StreamVariant {
        if self.variants.is_empty() {
            panic!("Master playlist has no variants");
        }

        // If no bandwidth information, return the first one (often used as default/auto)
        if available_bandwidth_bps == 0 {
            return &self.variants[0];
        }

        // Choose the highest bandwidth variant that is <= 80% of available bandwidth
        let target_bandwidth = (available_bandwidth_bps as f64 * 0.8) as u64;

        let mut best_variant = &self.variants[0];
        let mut max_bandwidth = 0;

        for variant in &self.variants {
            tracing::trace!(
                available_bandwidth_bps,
                variant = variant.bandwidth,
                "considering variant"
            );
            if variant.bandwidth <= target_bandwidth && variant.bandwidth > max_bandwidth {
                max_bandwidth = variant.bandwidth;
                best_variant = variant;
            }
        }

        // If none fit the 80% rule, just return the lowest bandwidth one
        if max_bandwidth == 0 {
            let mut lowest_bandwidth = u64::MAX;
            for variant in &self.variants {
                if variant.bandwidth < lowest_bandwidth {
                    lowest_bandwidth = variant.bandwidth;
                    best_variant = variant;
                }
            }
        }

        tracing::debug!(?best_variant, "selected best variant");

        best_variant
    }
}
