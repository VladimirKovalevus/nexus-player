use super::{artwork_source_data, ensure_artwork_on, LibraryService};
use anyhow::Result;

impl LibraryService {
    pub async fn ensure_artwork_url(&self, url: &str) -> Result<i64> {
        ensure_artwork_on(&self.db, url).await
    }

    /// Which of `urls` already belong to something saved in the library.
    ///
    /// An `artworks_v2` row exists only for a saved track or playlist (see the
    /// `ensure_artwork` call sites), so this is the test for "is in the library" —
    /// used to keep transient listings like search results from pulling traffic.
    pub async fn known_artwork_urls(
        &self,
        urls: &[String],
    ) -> Result<std::collections::HashSet<String>> {
        use crate::library::entities_v2::artwork;
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};

        let mut known = std::collections::HashSet::new();
        if urls.is_empty() {
            return Ok(known);
        }

        // Chunked to stay clear of SQLite's bound-parameter ceiling.
        for chunk in urls.chunks(400) {
            let encoded: Vec<String> = chunk.iter().map(|u| artwork_source_data(u)).collect();

            let found: Vec<String> = artwork::Entity::find()
                .filter(artwork::Column::SourceType.eq("url"))
                .filter(artwork::Column::SourceData.is_in(encoded))
                .select_only()
                .column(artwork::Column::SourceData)
                .into_tuple()
                .all(&self.db)
                .await?;

            let found: std::collections::HashSet<String> = found.into_iter().collect();
            known.extend(
                chunk
                    .iter()
                    .filter(|u| found.contains(&artwork_source_data(u)))
                    .cloned(),
            );
        }

        Ok(known)
    }
}
