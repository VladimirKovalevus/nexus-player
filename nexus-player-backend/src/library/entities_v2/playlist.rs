use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "playlists_v2")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,

    #[sea_orm(column_type = "Text")]
    pub title: String,

    #[sea_orm(column_type = "Text", nullable)]
    pub description: Option<String>,

    #[sea_orm(column_type = "Text", nullable)]
    pub creator: Option<String>,

    pub artwork_id: Option<i64>,

    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::artwork::Entity",
        from = "Column::ArtworkId",
        to = "super::artwork::Column::Id"
    )]
    Artwork,

    #[sea_orm(has_many = "super::playlist_track::Entity")]
    PlaylistTracks,
}

impl Related<super::artwork::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Artwork.def()
    }
}

impl Related<super::playlist_track::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PlaylistTracks.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
