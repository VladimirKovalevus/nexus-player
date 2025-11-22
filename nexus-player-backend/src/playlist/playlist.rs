use std::time::Duration;

use crate::{audio::types::Track, library};

#[derive(Debug, Clone)]
pub struct Playlist {
    //#TODO METADATA
    pub title: String,
    pub description: Option<String>,
    pub artwork: Option<String>,
    pub tracks: Vec<Track>,
}

impl Playlist {
    pub fn new(title: String) -> Self {
        Self {
            tracks: Vec::new(),
            title: title,
            description: None,
            artwork: None,
        }
    }

    pub fn add_track(&mut self, mut track: Track) -> usize {
        if track.id == 0 {
            track.id = self.tracks.len();
        }

        self.tracks.push(track);

        self.tracks.len() - 1
    }

    pub fn add_tracks(&mut self, tracks: Vec<Track>) {
        for track in tracks {
            self.add_track(track);
        }
    }

    pub fn remove_track(&mut self, index: usize) -> Option<Track> {
        if index < self.tracks.len() {
            Some(self.tracks.remove(index))
        } else {
            None
        }
    }

    pub fn remove_track_by_id(&mut self, track_id: usize) -> Option<Track> {
        if let Some(index) = self.tracks.iter().position(|t| t.id == track_id) {
            self.remove_track(index)
        } else {
            None
        }
    }

    pub fn clear(&mut self) {
        self.tracks.clear();
    }

    pub fn get_track(&self, index: usize) -> Option<&Track> {
        self.tracks.get(index)
    }

    pub fn get_track_mut(&mut self, index: usize) -> Option<&mut Track> {
        self.tracks.get_mut(index)
    }

    pub fn find_track_by_id(&self, track_id: usize) -> Option<&Track> {
        self.tracks.iter().find(|t| t.id == track_id)
    }

    pub fn find_index_by_id(&self, track_id: usize) -> Option<usize> {
        self.tracks.iter().position(|t| t.id == track_id)
    }

    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }

    pub fn len(&self) -> usize {
        self.tracks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }
}
