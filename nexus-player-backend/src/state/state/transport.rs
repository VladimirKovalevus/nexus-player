use super::AppState;
use crate::audio::eq::{builtin_presets, EqSettings};
use std::time::Duration;

impl AppState {
    // --- Audio Control Proxies ---

    pub fn pause(&self) {
        self.audio_engine.pause();
    }
    pub fn resume(&self) {
        self.audio_engine.resume();
    }
    pub fn stop(&self) {
        self.audio_engine.stop();
    }
    pub fn seek(&self, position: Duration) {
        self.audio_engine.seek(position);
    }
    pub fn set_volume(&self, volume: f32) {
        self.audio_engine.set_volume(volume.clamp(0.0, 1.0));
    }
    pub fn audio_state(&self) -> crate::audio::types::AudioState {
        self.audio_engine.state()
    }
    pub fn poll_audio_events(&self) -> Vec<crate::audio::types::AudioEvent> {
        self.audio_engine.poll_all_events()
    }

    pub fn set_eq_settings(&self, settings: EqSettings) -> anyhow::Result<()> {
        self.audio_engine.set_eq(settings.clone());
        let mut config = self.config_manager.get();
        config.eq = settings;
        self.config_manager.update(config)?;
        Ok(())
    }

    pub fn get_eq_settings(&self) -> EqSettings {
        self.config_manager.get().eq
    }

    pub fn get_eq_presets(&self) -> Vec<(String, [f32; 10])> {
        builtin_presets()
            .into_iter()
            .map(|(name, gains)| (name.to_string(), gains))
            .collect()
    }
}
