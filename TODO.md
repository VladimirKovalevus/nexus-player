# Nexus Player Roadmap & TODO

## 🎵 Core Features & Logic

- [x] **Smart Cover Art Fallback**
  - [x] Logic: When creating a track from an external source (playlist/album), if the track has no specific cover, inherit the cover from the parent playlist.
  - [x] Implementation Area: `nexus-player-backend/src/library/` (likely `metadata.rs` or `service.rs`).

- [x] **Streaming Prefetch & Buffering**
  - [x] **Logic**: Design and implement logic to prefetch the next track in the queue to reduce latency between tracks.
  - [ ] **Buffering**: Optimize buffering strategies for smoother playback (avoid stalls on network fluctuations).
  - [x] **Implementation Area**: `nexus-player-backend/src/streaming/` and `nexus-player-backend/src/audio/`.

- [ ] **Meta-Playlists (Nested Playlists)**
  - [ ] **Logic**: Allow adding playlists into another playlist.
  - [ ] **Playback**: When playing a meta-playlist, the player should traverse through the contained playlists sequentially.

- [x] **Cursor-based Playback Queue**
  - [x] **Structure**: Implement a `Cursor` struct for `PlaybackQueue` that abstracts the source of tracks.
  - [x] **Context Awareness**:
    - [x] *Search Results*: Iterate sequentially through search results.
    - [x] *Database/Library*: Fetch tracks lazily/paginated from the DB as playback progresses (avoid loading huge lists into memory).
  - [x] **Implementation Area**: `nexus-player-backend/src/playlist/playback_queue.rs`.

- [ ] **Like / Favorite System**
  - [ ] **Database**: Add schema for "Liked" tracks (boolean flag on Track entity or separate `favorites` table).
  - [ ] **API**: Add endpoints to toggle Like status.
  - [ ] **Optimization**: Implement a "cheap" lookup mechanism during Search to identify if a result is already in the DB/liked without fetching full objects (e.g., return a Set of IDs or a specific status field in search results).
  - [ ] **UI**: Add Heart icon next to tracks. Toggle visual state immediately on click (optimistic UI).

## 🎨 UI/UX Polish (Frontend)

- [ ] **Playback State Synchronization**
  - [ ] Ensure Frontend always reflects the actual player state.
  - [ ] **Transition Logic**: When changing tracks:
    - [ ] Stop current track immediately.
    - [ ] Display next track metadata immediately.
    - [ ] Show a "Loading/Buffering" placeholder state if audio isn't ready yet.
  - [ ] Implementation: `nexus-player-frontend/src/stores/audio.ts`.

- [ ] **Album & Playlist Interaction**
  - [ ] **Click Behavior**:
    - [ ] Clicking the container/art: Open a modal/view showing tracklist and details.
    - [ ] Clicking "Play" button: Replace queue with this album/playlist and start playback.
  - [ ] Implementation: `MediaCard.vue` and related components.

## 🔌 Integrations

- [ ] **VK Music Integration**
  - [ ] **Auth**: Implement Authentication flow (check existing library capabilities).
  - [ ] **Provider**: Create `nexus-player-backend/src/streaming/providers/vk.rs`.
  - [ ] **Migration**: Add ability to import/migrate tracks from VK.

- [ ] **SoundCloud Debugging & Optimization**
  - [ ] **Remove yt-dlp Dependency**: Refactor `soundcloud.rs` to fetch audio streams directly from SoundCloud API/Internal API instead of using `yt-dlp`.
  - [ ] **Investigation**: Identify specific tracks that fail to play.
  - [ ] **Fix**: Debug `soundcloud.rs` logic.
  - [ ] **Data Collection**: Find examples of broken URLs.

## 📱 Mobile & Build (Android)

- [ ] **Android Build Config**
  - [ ] Fix current build issues to ensure a successful compilation.
  - [ ] **Size Optimization**:
    - [ ] Enable LTO (Link Time Optimization) in `Cargo.toml`.
    - [ ] Strip symbols.
    - [ ] Review included assets/libs to reduce APK size.

## ⚙️ System Configuration & Extensibility

- [ ] **Global Configuration System**
  - [ ] **Logic**: Implement a configuration loading mechanism (e.g., YAML/JSON/TOML) that initializes at application startup.
  - [ ] **Configurable Parameters**:
    - [ ] **Provider Management**: Toggle specific streaming providers (YouTube, SoundCloud, etc.) on/off.
    - [ ] **Audio Engine**: Define `chunk_size` for chunked audio playback and buffering.
    - [ ] **Network**: Set global `timeout` for API requests and media streaming to handle slow connections.
  - [ ] **Implementation Area**: `nexus-player-backend/src/state/` or a new configuration module.

## 🧹 Refactoring

- [ ] Cleanup legacy code in `nexus-player-backend/src/library/` (v1 vs v2 files).
