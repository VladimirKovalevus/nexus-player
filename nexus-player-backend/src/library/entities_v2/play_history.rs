use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "play_history")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,

    pub title: String,
    pub artist: Option<String>,

    #[sea_orm(column_type = "Text")]
    pub source_type: String,

    #[sea_orm(column_type = "Text")]
    pub source_data: String,

    pub artwork_url: Option<String>,
    pub played_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
