use biquad::{Biquad, Coefficients, DirectForm2Transposed, ToHertz, Type};
use serde::{Deserialize, Serialize};

const BAND_FREQS: [f32; 10] = [
    32.0, 64.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
];
const SHELF_Q: f32 = 0.707;
const PEAK_Q: f32 = 1.4;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EqSettings {
    pub enabled: bool,
    pub gains: [f32; 10],
    #[serde(deserialize_with = "deserialize_preset_name")]
    pub preset_name: Option<String>,
}

impl Default for EqSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            gains: [0.0; 10],
            preset_name: Some("flat".to_string()),
        }
    }
}

const PRESET_IDS: [&str; 7] = [
    "flat",
    "rock",
    "pop",
    "jazz",
    "classical",
    "bass_boost",
    "vocal",
];

// Names used before presets got stable ids, in the same order as PRESET_IDS.
const LEGACY_PRESET_NAMES: [&str; 7] = [
    "\u{41f}\u{43b}\u{43e}\u{441}\u{43a}\u{438}\u{439}",
    "\u{420}\u{43e}\u{43a}",
    "\u{41f}\u{43e}\u{43f}",
    "\u{414}\u{436}\u{430}\u{437}",
    "\u{41a}\u{43b}\u{430}\u{441}\u{441}\u{438}\u{43a}\u{430}",
    "\u{411}\u{430}\u{441}-\u{431}\u{443}\u{441}\u{442}",
    "\u{412}\u{43e}\u{43a}\u{430}\u{43b}",
];

fn migrate_preset_name(name: String) -> Option<String> {
    if PRESET_IDS.contains(&name.as_str()) {
        return Some(name);
    }
    LEGACY_PRESET_NAMES
        .iter()
        .position(|legacy| *legacy == name)
        .map(|i| PRESET_IDS[i].to_string())
}

fn deserialize_preset_name<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Option::<String>::deserialize(deserializer)?;
    Ok(raw.and_then(migrate_preset_name))
}

pub fn builtin_presets() -> Vec<(&'static str, [f32; 10])> {
    vec![
        ("flat", [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
        ("rock", [4.0, 3.0, 0.0, -1.0, -2.0, 0.0, 2.0, 3.0, 3.0, 2.0]),
        (
            "pop",
            [-1.0, 2.0, 4.0, 4.0, 1.0, 0.0, -1.0, -1.0, -1.0, -2.0],
        ),
        ("jazz", [3.0, 2.0, 1.0, 2.0, -1.0, -1.0, 0.0, 1.0, 2.0, 3.0]),
        (
            "classical",
            [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -4.0, -4.0, -4.0, -5.0],
        ),
        (
            "bass_boost",
            [6.0, 5.0, 4.0, 2.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        ),
        (
            "vocal",
            [-3.0, -3.0, 0.0, 3.0, 5.0, 5.0, 3.0, 0.0, -1.0, -2.0],
        ),
    ]
}

// Per-channel biquad state for a single band.
type BandFilters = Vec<DirectForm2Transposed<f32>>;

/// Stateful EQ processor — lives entirely inside the CPAL output callback closure.
/// State (z1, z2) is never shared; gains/enabled are received via AtomicBool dirty flag.
pub struct EqProcessor {
    // filters[band][channel]
    filters: Vec<BandFilters>,
    pub enabled: bool,
    sample_rate: u32,
    channels: usize,
}

impl EqProcessor {
    pub fn new(settings: &EqSettings, sample_rate: u32, channels: usize) -> Self {
        let ch = channels.max(1);
        let filters = (0..10)
            .map(|band| {
                (0..ch)
                    .map(|_| build_filter(band, settings.gains[band], sample_rate))
                    .collect()
            })
            .collect();

        Self {
            filters,
            enabled: settings.enabled,
            sample_rate,
            channels: ch,
        }
    }

    /// Rebuild filter coefficients from new settings (preserves state continuity — z1/z2 are kept,
    /// so there's a brief transient on update but no audible pop at moderate gain changes).
    pub fn rebuild(&mut self, settings: &EqSettings, sample_rate: u32) {
        self.enabled = settings.enabled;
        self.sample_rate = sample_rate;
        for band in 0..10 {
            for ch in 0..self.channels {
                self.filters[band][ch] = build_filter(band, settings.gains[band], sample_rate);
            }
        }
    }

    /// Process interleaved PCM buffer in-place.
    pub fn process(&mut self, data: &mut [f32], channels: usize) {
        if !self.enabled {
            return;
        }
        let ch_count = channels.min(self.channels);
        let frame_count = data.len() / channels;
        for frame in 0..frame_count {
            for ch in 0..ch_count {
                let idx = frame * channels + ch;
                let mut s = data[idx];
                for band in 0..10 {
                    s = self.filters[band][ch].run(s);
                }
                data[idx] = s;
            }
        }
    }

    pub fn current_sample_rate(&self) -> u32 {
        self.sample_rate
    }
}

fn build_filter(band_idx: usize, gain_db: f32, sample_rate: u32) -> DirectForm2Transposed<f32> {
    let sr = sample_rate as f32;
    // Clamp freq below Nyquist so the biquad formula stays numerically stable.
    let freq = BAND_FREQS[band_idx].min(sr * 0.45);

    let filter_type: Type<f32> = if band_idx == 0 {
        Type::LowShelf(gain_db)
    } else if band_idx == 9 {
        Type::HighShelf(gain_db)
    } else {
        Type::PeakingEQ(gain_db)
    };
    let q = if band_idx == 0 || band_idx == 9 {
        SHELF_Q
    } else {
        PEAK_Q
    };

    match Coefficients::<f32>::from_params(filter_type, sr.hz(), freq.hz(), q) {
        Ok(coeffs) => DirectForm2Transposed::<f32>::new(coeffs),
        Err(_) => {
            // Numerically degenerate case (e.g. sample_rate extremely low): pass-through
            let flat = Coefficients::<f32>::from_params(
                Type::PeakingEQ(0.0),
                48000.0_f32.hz(),
                1000.0_f32.hz(),
                1.0,
            )
            .expect("fallback filter params are always valid");
            DirectForm2Transposed::<f32>::new(flat)
        }
    }
}
