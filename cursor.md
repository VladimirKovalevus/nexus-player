# Cursor & Meta-Playlist Architecture

## Overview
We are moving from a static `Vec<Track>` playback queue to a dynamic, lazy-loading **Cursor-based** system. This allows for:
1. **Infinite Contexts**: Radio stations, infinite search results.
2. **Nested Structures**: Playing a search result that contains an Album, which "expands" seamlessly without losing the search context.
3. **Memory Efficiency**: Loading tracks only when needed.

## Core Concepts

### 1. CursorItem
The atomic unit returned by a cursor. It can be a playable track or a container that needs expansion.

```rust
pub enum CursorItem {
    Track(Track),                // Ready to play
    Container(ContainerContext), // Album/Playlist to expand
}
```

### 2. ContainerContext
Describes "what" we are playing.

```rust
pub enum ContainerContext {
    // Local DB
    LocalPlaylist(i64),
    LocalAlbum(i64),
    
    // External Providers
    ExternalPlaylist { system: String, id: String },
    ExternalAlbum { system: String, id: String },
    
    // Dynamic
    SearchQuery { system: String, query: String },
}
```

### 3. CursorSource (Trait)
An async iterator that knows how to fetch the next batch of items from a specific context.

```rust
#[async_trait]
pub trait CursorSource: Send + Sync {
    async fn next_batch(&mut self, limit: usize) -> anyhow::Result<Vec<CursorItem>>;
    // Optional: serialization for state restoration
    // fn save_state(&self) -> serde_json::Value; 
}
```

### 4. PlayerCursor (The Engine)
Manages a **Stack** of cursors to handle nested contexts (DFS traversal).

*Structure:*
- `stack: Vec<Box<dyn CursorSource>>`

*Logic for `next_track()`:*
1. Peek top cursor.
2. Fetch item.
3. If `Track` -> Return it.
4. If `Container` -> 
   - Pause current cursor.
   - Create NEW cursor for container.
   - Push to stack.
   - Recursive call.
5. If `EOF` -> Pop stack. If stack empty -> Stop.

### 5. MetaPlaylist
The high-level structure managing the entire session.

```rust
pub struct MetaPlaylist {
    pub id: Uuid,
    pub manual_queue: VecDeque<Track>, // User-added tracks (Priority 1)
    pub cursor: PlayerCursor,          // Generated stream (Priority 2)
    pub history: Vec<Track>,           // For "Prev" button
}
```

## Implementation Plan

### Phase 1: Type Definitions
- Create `nexus-player-backend/src/playlist/cursor/`.
- Define `CursorItem`, `ContainerContext`.
- Define `CursorSource` trait.

### Phase 2: PlayerCursor Logic
- Implement the stack management.
- Implement the "expansion" logic (resolving `ContainerContext` to a new `CursorSource`).

### Phase 3: Concrete Sources
- `DbTrackCursor`: Iterates DB query.
- `StreamingSearchCursor`: Wraps `StreamingService::search`.

### Phase 4: Integration
- Replace internal logic of `PlaybackQueue` or wrap it.
- Update `AppState` to support initializing playback from a Context.

### Phase 5: UI & Prefetching
- Expose `peek()` to frontend.
- Visualize "Loading..." states for upcoming lazy tracks.
