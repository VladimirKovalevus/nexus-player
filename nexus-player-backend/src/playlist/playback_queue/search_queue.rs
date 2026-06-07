use super::*;

impl SearchQueue {
    pub fn new(system: String, query: String, items: Vec<SearchQueueItem>, has_more: bool) -> Self {
        let offset = items.len();
        Self {
            system,
            query,
            items,
            current_index: None,
            search_offset: offset,
            has_more_pages: has_more,
            repeat_mode: RepeatMode::Off,
            shuffle_enabled: false,
            shuffle_order: Vec::new(),
        }
    }

    pub fn toggle_shuffle(&mut self) {
        self.shuffle_enabled = !self.shuffle_enabled;
        if self.shuffle_enabled {
            self.regenerate_shuffle();
        } else {
            // Re-sync current_index to the actual items index
            if let Some(shuffle_idx) = self.current_index {
                if shuffle_idx < self.shuffle_order.len() {
                    self.current_index = Some(self.shuffle_order[shuffle_idx]);
                }
            }
            self.shuffle_order.clear();
        }
    }

    fn regenerate_shuffle(&mut self) {
        let mut indices: Vec<usize> = (0..self.items.len()).collect();
        indices.shuffle(&mut thread_rng());

        // Find where current item went
        let current_real_idx = if let Some(idx) = self.current_index {
            if self.shuffle_order.is_empty() {
                Some(idx)
            } else if idx < self.shuffle_order.len() {
                Some(self.shuffle_order[idx])
            } else {
                None
            }
        } else {
            None
        };

        self.shuffle_order = indices;

        if let Some(real_idx) = current_real_idx {
            self.current_index = self.shuffle_order.iter().position(|&i| i == real_idx);
        }
    }

    pub fn jump_to(&mut self, index: usize) {
        if self.shuffle_enabled {
            if index < self.shuffle_order.len() {
                self.current_index = Some(index);
                let real_idx = self.shuffle_order[index];
                if let Some(SearchQueueItem::Container { play_index, .. }) =
                    self.items.get_mut(real_idx)
                {
                    *play_index = 0;
                }
            }
        } else if index < self.items.len() {
            self.current_index = Some(index);
            // Reset play_index for containers
            if let Some(SearchQueueItem::Container { play_index, .. }) = self.items.get_mut(index) {
                *play_index = 0;
            }
        }
    }

    pub fn jump_to_with_track_index(&mut self, index: usize, track_idx: usize) {
        // Here 'index' is index in the visible list (snapshot)
        // If shuffle is enabled, we need to handle it.
        // But usually snapshot() returns tracks in shuffle order if enabled.
        // Let's assume index is already the "shuffle" index if shuffle is on.

        let real_idx = if self.shuffle_enabled {
            if index < self.shuffle_order.len() {
                self.current_index = Some(index);
                self.shuffle_order[index]
            } else {
                return;
            }
        } else if index < self.items.len() {
            self.current_index = Some(index);
            index
        } else {
            return;
        };

        if let Some(SearchQueueItem::Container {
            play_index,
            expanded_tracks: Some(tracks),
            ..
        }) = self.items.get_mut(real_idx)
        {
            if track_idx < tracks.len() {
                *play_index = track_idx;
            }
        }
    }

    pub fn current_track(&self) -> Option<Track> {
        let idx = self.current_index?;
        let real_idx = if self.shuffle_enabled {
            *self.shuffle_order.get(idx)?
        } else {
            idx
        };

        match self.items.get(real_idx)? {
            SearchQueueItem::Track(track) => Some(track.clone()),
            SearchQueueItem::Container {
                expanded_tracks,
                play_index,
                ..
            } => expanded_tracks.as_ref()?.get(*play_index).cloned(),
        }
    }

    pub fn peek_upcoming_tracks(&self, limit: usize) -> Vec<Track> {
        let current_shuffle_idx = match self.current_index {
            Some(i) => i,
            None => return vec![],
        };

        let mut result = Vec::with_capacity(limit);

        // 1. Current container remainder
        let real_idx = if self.shuffle_enabled {
            self.shuffle_order[current_shuffle_idx]
        } else {
            current_shuffle_idx
        };

        if let Some(SearchQueueItem::Container {
            expanded_tracks: Some(tracks),
            play_index,
            ..
        }) = self.items.get(real_idx)
        {
            for track in &tracks[(*play_index + 1).min(tracks.len())..] {
                if result.len() >= limit {
                    return result;
                }
                result.push(track.clone());
            }
        }

        // 2. Subsequent items
        let next_start = current_shuffle_idx + 1;
        let total_len = if self.shuffle_enabled {
            self.shuffle_order.len()
        } else {
            self.items.len()
        };

        for i in next_start..total_len {
            if result.len() >= limit {
                break;
            }
            let idx = if self.shuffle_enabled {
                self.shuffle_order[i]
            } else {
                i
            };
            match &self.items[idx] {
                SearchQueueItem::Track(track) => result.push(track.clone()),
                SearchQueueItem::Container {
                    expanded_tracks: Some(tracks),
                    ..
                } => {
                    for track in tracks {
                        if result.len() >= limit {
                            break;
                        }
                        result.push(track.clone());
                    }
                }
                _ => {}
            }
        }

        result
    }

    pub fn peek_next(&self) -> Option<Track> {
        let current_shuffle_idx = self.current_index?;

        // 1. If current is a container, check if there's a next track within it
        let real_idx = if self.shuffle_enabled {
            self.shuffle_order[current_shuffle_idx]
        } else {
            current_shuffle_idx
        };

        if let Some(SearchQueueItem::Container {
            expanded_tracks: Some(tracks),
            play_index,
            ..
        }) = self.items.get(real_idx)
        {
            if *play_index + 1 < tracks.len() {
                return Some(tracks[*play_index + 1].clone());
            }
        }

        // 2. Otherwise, check the next item in the queue
        let next_shuffle_idx = current_shuffle_idx + 1;
        let total_len = if self.shuffle_enabled {
            self.shuffle_order.len()
        } else {
            self.items.len()
        };

        if next_shuffle_idx < total_len {
            let next_real_idx = if self.shuffle_enabled {
                self.shuffle_order[next_shuffle_idx]
            } else {
                next_shuffle_idx
            };
            match &self.items[next_real_idx] {
                SearchQueueItem::Track(track) => return Some(track.clone()),
                SearchQueueItem::Container {
                    expanded_tracks: Some(tracks),
                    ..
                } if !tracks.is_empty() => {
                    return Some(tracks[0].clone());
                }
                _ => {}
            }
        }

        None
    }

    pub fn advance(&mut self) -> NextResult {
        // Handle Repeat One
        if self.repeat_mode == RepeatMode::One {
            if let Some(track) = self.current_track() {
                return NextResult::Track(track);
            }
        }

        let current_shuffle_idx = match self.current_index {
            Some(idx) => idx,
            None => {
                // Start from the beginning
                if self.items.is_empty() {
                    if self.has_more_pages {
                        return NextResult::NeedMorePages;
                    }
                    return NextResult::End;
                }

                if self.shuffle_enabled && self.shuffle_order.is_empty() {
                    self.regenerate_shuffle();
                }

                self.current_index = Some(0);
                let first_real_idx = if self.shuffle_enabled {
                    self.shuffle_order[0]
                } else {
                    0
                };
                return self.return_current_or_expand(first_real_idx);
            }
        };

        // If current is a container with tracks, try advancing within it
        let real_idx = if self.shuffle_enabled {
            self.shuffle_order[current_shuffle_idx]
        } else {
            current_shuffle_idx
        };

        if let Some(SearchQueueItem::Container {
            expanded_tracks: Some(tracks),
            play_index,
            ..
        }) = self.items.get(real_idx)
        {
            if *play_index + 1 < tracks.len() {
                // Advance within container
                if let Some(SearchQueueItem::Container {
                    play_index,
                    expanded_tracks: Some(tracks),
                    ..
                }) = self.items.get_mut(real_idx)
                {
                    *play_index += 1;
                    return NextResult::Track(tracks[*play_index].clone());
                }
            }
        }

        // Move to next item
        let next_shuffle_idx = current_shuffle_idx + 1;
        let total_len = if self.shuffle_enabled {
            self.shuffle_order.len()
        } else {
            self.items.len()
        };

        if next_shuffle_idx >= total_len {
            if self.has_more_pages {
                return NextResult::NeedMorePages;
            }

            // Handle Repeat All
            if self.repeat_mode == RepeatMode::All && !self.items.is_empty() {
                if self.shuffle_enabled {
                    self.regenerate_shuffle();
                }
                self.current_index = Some(0);
                let first_real_idx = if self.shuffle_enabled {
                    self.shuffle_order[0]
                } else {
                    0
                };

                // Ensure that if the first item is a container, it starts from the beginning
                if let Some(SearchQueueItem::Container { play_index, .. }) =
                    self.items.get_mut(first_real_idx)
                {
                    *play_index = 0;
                }
                return self.return_current_or_expand(first_real_idx);
            }
            return NextResult::End;
        }

        self.current_index = Some(next_shuffle_idx);
        let next_real_idx = if self.shuffle_enabled {
            self.shuffle_order[next_shuffle_idx]
        } else {
            next_shuffle_idx
        };

        // Ensure that if the next item is a container, it starts from the beginning
        if let Some(SearchQueueItem::Container { play_index, .. }) =
            self.items.get_mut(next_real_idx)
        {
            *play_index = 0;
        }

        self.return_current_or_expand(next_real_idx)
    }

    pub fn previous(&mut self) -> NextResult {
        // Handle Repeat One
        if self.repeat_mode == RepeatMode::One {
            if let Some(track) = self.current_track() {
                return NextResult::Track(track);
            }
        }

        let current_shuffle_idx = match self.current_index {
            Some(idx) => idx,
            None => return NextResult::End,
        };

        let real_idx = if self.shuffle_enabled {
            self.shuffle_order[current_shuffle_idx]
        } else {
            current_shuffle_idx
        };

        // If current is a container with tracks, try going back within it
        if let Some(SearchQueueItem::Container {
            expanded_tracks: Some(_),
            play_index,
            ..
        }) = self.items.get(real_idx)
        {
            if *play_index > 0 {
                if let Some(SearchQueueItem::Container {
                    play_index,
                    expanded_tracks: Some(tracks),
                    ..
                }) = self.items.get_mut(real_idx)
                {
                    *play_index -= 1;
                    return NextResult::Track(tracks[*play_index].clone());
                }
            }
        }

        // Move to previous item
        if current_shuffle_idx == 0 {
            // Handle Repeat All
            if self.repeat_mode == RepeatMode::All && !self.items.is_empty() {
                let last_shuffle_idx = (if self.shuffle_enabled {
                    self.shuffle_order.len()
                } else {
                    self.items.len()
                }) - 1;
                self.current_index = Some(last_shuffle_idx);
                let last_real_idx = if self.shuffle_enabled {
                    self.shuffle_order[last_shuffle_idx]
                } else {
                    last_shuffle_idx
                };

                // If the last item is a container, land on its last track
                if let Some(SearchQueueItem::Container {
                    play_index,
                    expanded_tracks,
                    ..
                }) = self.items.get_mut(last_real_idx)
                {
                    if let Some(tracks) = expanded_tracks {
                        *play_index = tracks.len().saturating_sub(1);
                    } else {
                        *play_index = 0;
                    }
                }
                return self.return_current_or_expand(last_real_idx);
            }

            return NextResult::End;
        }

        let prev_shuffle_idx = current_shuffle_idx - 1;
        self.current_index = Some(prev_shuffle_idx);
        let prev_real_idx = if self.shuffle_enabled {
            self.shuffle_order[prev_shuffle_idx]
        } else {
            prev_shuffle_idx
        };

        // If the previous item is a container, we usually want its LAST track when going backwards
        if let Some(SearchQueueItem::Container {
            play_index,
            expanded_tracks,
            ..
        }) = self.items.get_mut(prev_real_idx)
        {
            if let Some(tracks) = expanded_tracks {
                *play_index = tracks.len().saturating_sub(1);
            } else {
                *play_index = 0;
            }
        }

        self.return_current_or_expand(prev_real_idx)
    }

    fn return_current_or_expand(&self, idx: usize) -> NextResult {
        match &self.items[idx] {
            SearchQueueItem::Track(track) => NextResult::Track(track.clone()),
            SearchQueueItem::Container {
                expanded_tracks: None,
                ..
            } => NextResult::NeedExpand(idx),
            SearchQueueItem::Container {
                expanded_tracks: Some(tracks),
                play_index,
                ..
            } => {
                if tracks.is_empty() {
                    // Empty container, skip conceptually — but return End for now
                    NextResult::End
                } else {
                    NextResult::Track(tracks[*play_index].clone())
                }
            }
        }
    }

    pub fn set_container_tracks(&mut self, index: usize, tracks: Vec<Track>) {
        if let Some(SearchQueueItem::Container {
            expanded_tracks,
            play_index,
            ..
        }) = self.items.get_mut(index)
        {
            *expanded_tracks = Some(tracks);
            *play_index = 0;
        }
    }

    pub fn append_results(&mut self, new_items: Vec<SearchQueueItem>, has_more: bool) {
        let old_len = self.items.len();
        self.search_offset += new_items.len();
        self.items.extend(new_items);
        self.has_more_pages = has_more;

        if self.shuffle_enabled {
            let mut new_indices: Vec<usize> = (old_len..self.items.len()).collect();
            new_indices.shuffle(&mut thread_rng());
            self.shuffle_order.extend(new_indices);
        }
    }

    pub fn snapshot(&self) -> (Vec<QueueEntry>, Option<CurrentPosition>) {
        let items_to_show: Vec<&SearchQueueItem> =
            if self.shuffle_enabled && !self.shuffle_order.is_empty() {
                self.shuffle_order
                    .iter()
                    .map(|&idx| &self.items[idx])
                    .collect()
            } else {
                self.items.iter().collect()
            };

        let entries: Vec<QueueEntry> = items_to_show
            .iter()
            .map(|item| match item {
                SearchQueueItem::Track(track) => QueueEntry::Track(track.clone()),
                SearchQueueItem::Container {
                    info,
                    expanded_tracks,
                    play_index,
                } => QueueEntry::Container {
                    info: info.clone(),
                    tracks: expanded_tracks.clone(),
                    play_index: *play_index,
                },
            })
            .collect();

        let position = self.current_index.map(|idx| {
            let real_idx = if self.shuffle_enabled && !self.shuffle_order.is_empty() {
                self.shuffle_order.get(idx).copied().unwrap_or(idx)
            } else {
                idx
            };

            let track_within = match self.items.get(real_idx) {
                Some(SearchQueueItem::Container {
                    expanded_tracks: Some(_),
                    play_index,
                    ..
                }) => Some(*play_index),
                _ => None,
            };
            CurrentPosition {
                item_index: idx,
                track_within_container: track_within,
            }
        });

        (entries, position)
    }
}
