use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "artworks_v2")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,

    #[sea_orm(column_type = "Text")]
    pub source_type: String,

    #[sea_orm(column_type = "Text", nullable)]
    pub source_data: Option<String>,

    pub width: Option<i32>,
    pub height: Option<i32>,

    pub created_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::track::Entity")]
    Tracks,
    #[sea_orm(has_many = "super::playlist::Entity")]
    Playlists,
}

impl Related<super::track::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Tracks.def()
    }
}

impl Related<super::playlist::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Playlists.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
