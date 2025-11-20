





use sqlx::FromRow;






#[derive(Debug, Clone, FromRow)]
pub(super) struct ArtworkRow {
    pub id: i64,

    
    pub data: Option<Vec<u8>>,
    pub url: Option<String>,

    
    pub mime_type: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,

    pub created_at: i64,
}






#[derive(Debug, Clone, FromRow)]
pub(super) struct TrackRow {
    pub id: i64,

    
    pub title: String,
    pub artist: Option<String>,
    pub description: Option<String>,
    pub artwork_id: Option<i64>,

    
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub year: Option<i32>,
    pub genre: Option<String>,
    pub track_number: Option<i32>,

    
    pub source_type: String,  
    pub source_local_path: Option<String>,
    pub source_http_url: Option<String>,
    pub source_external_system: Option<String>,
    pub source_external_id: Option<String>,

    
    pub cached_local_path: Option<String>,
    pub cache_status: String,  
    pub cache_downloaded_at: Option<i64>,

    
    pub file_format: Option<String>,
    pub duration_secs: Option<f64>,
    pub sample_rate: Option<i32>,
    pub channels: Option<i32>,
    pub bitrate: Option<i32>,

    
    pub play_count: i64,
    pub last_played: Option<i64>,

    
    pub added_at: i64,
    pub updated_at: i64,
}






#[derive(Debug, Clone, FromRow)]
pub(super) struct PlaylistRow {
    pub id: i64,

    
    pub title: String,
    pub description: Option<String>,
    pub creator: Option<String>,
    pub artwork_id: Option<i64>,

    
    pub created_at: i64,
    pub updated_at: i64,
}


#[derive(Debug, Clone, FromRow)]
pub(super) struct PlaylistTrackRow {
    pub playlist_id: i64,
    pub track_id: i64,
    pub position: i64,
    pub added_at: i64,
}
