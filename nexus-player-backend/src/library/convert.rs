use crate::audio::types;
use crate::library::models;

impl From<models::Track> for types::Track {
    fn from(track: models::Track) -> Self {
        types::Track {
            id: track.id.0 as usize,
            format: None,
            source: track.source,
        }
    }
}

impl From<&models::Track> for types::Track {
    fn from(track: &models::Track) -> Self {
        types::Track {
            id: track.id.0 as usize,
            format: None,
            source: track.source.clone(),
        }
    }
}
