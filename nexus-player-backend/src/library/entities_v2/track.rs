use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "tracks_v2")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,

    #[sea_orm(column_type = "Text")]
    pub title: String,

    #[sea_orm(column_type = "Text", nullable)]
    pub artist: Option<String>,

    #[sea_orm(column_type = "Text", nullable)]
    pub description: Option<String>,

    pub artwork_id: Option<i64>,

    #[sea_orm(column_type = "Text", nullable)]
    pub album: Option<String>,

    #[sea_orm(column_type = "Text", nullable)]
    pub album_artist: Option<String>,

    pub year: Option<i32>,

    #[sea_orm(column_type = "Text", nullable)]
    pub genre: Option<String>,

    pub track_number: Option<i32>,

    #[sea_orm(column_type = "Text")]
    pub source_type: String,

    #[sea_orm(column_type = "Text", nullable)]
    pub source_data: Option<String>,

    #[sea_orm(column_type = "Text")]
    pub cache_type: String,

    #[sea_orm(column_type = "Text", nullable)]
    pub cache_data: Option<String>,

    #[sea_orm(column_type = "Text", nullable)]
    pub file_format: Option<String>,

    pub duration_secs: Option<f64>,
    pub sample_rate: Option<i32>,
    pub channels: Option<i32>,
    pub bitrate: Option<i32>,

    pub play_count: i64,
    pub last_played: Option<i64>,

    pub added_at: i64,
    pub updated_at: i64,

    pub liked: i32,
    pub liked_position: Option<i64>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::artwork::Entity",
        from = "Column::ArtworkId",
        to = "super::artwork::Column::Id"
    )]
    Artwork,

}

impl Related<super::artwork::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Artwork.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
