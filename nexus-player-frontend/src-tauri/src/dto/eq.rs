use serde::{Deserialize, Serialize};

// ─── EQ DTOs ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EqSettingsDto {
    pub enabled: bool,
    pub gains: Vec<f32>,
    pub preset_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EqPresetDto {
    pub name: String,
    pub gains: Vec<f32>,
}

impl From<nexus_player_backend::audio::eq::EqSettings> for EqSettingsDto {
    fn from(s: nexus_player_backend::audio::eq::EqSettings) -> Self {
        Self {
            enabled: s.enabled,
            gains: s.gains.to_vec(),
            preset_name: s.preset_name,
        }
    }
}

impl From<EqSettingsDto> for nexus_player_backend::audio::eq::EqSettings {
    fn from(dto: EqSettingsDto) -> Self {
        let mut gains = [0.0f32; 10];
        for (i, &g) in dto.gains.iter().take(10).enumerate() {
            gains[i] = g.clamp(-12.0, 12.0);
        }
        Self {
            enabled: dto.enabled,
            gains,
            preset_name: dto.preset_name,
        }
    }
}
