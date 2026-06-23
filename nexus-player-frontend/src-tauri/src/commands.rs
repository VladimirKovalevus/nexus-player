type Result<T> = std::result::Result<T, String>;

pub(crate) mod cache;
pub(crate) mod config;
pub(crate) mod dialogs;
pub(crate) mod direct_link;
pub(crate) mod eq;
pub(crate) mod history;
pub(crate) mod library;
pub(crate) mod playlists;
pub(crate) mod queue;
pub(crate) mod queue_state;
pub(crate) mod search;
pub(crate) mod sync;
pub(crate) mod tracks;

pub(crate) use queue_state::radio_mode_label;
