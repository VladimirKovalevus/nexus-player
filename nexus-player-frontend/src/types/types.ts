export interface TrackDto {
  id: number;
  title: string;
  artist: string | null;
  album: string | null;
  durationSecs: number | null;
  artworkUrl: string | null;
  playCount: number;
  state: string;
}

export interface PlaylistDto {
  id: number;
  title: string;
  description: string | null;
  creator: string | null;
  track_count: number;
  created_at: number;
  updated_at: number;
}

export interface AudioState {
  state: 'playing' | 'paused' | 'stopped' | 'buffering';
  positionSecs: number;
  durationSecs: number;
  volume: number;
  currentTrack: TrackDto | null;
}

export interface QueueState {
  tracks: TrackDto[];
  currentIndex: number | null;
  repeatMode: 'off' | 'all' | 'one';
  shuffleEnabled: boolean;
}

export interface AudioEvent {
  type: string;
  payload?: any;
}

export interface StreamingTrack {
  type: 'track';
  title: string;
  artist: string;
  durationSecs?: number;
  thumbnail?: string;
  url: string;
}

export interface StreamingPlaylist {
  type: 'playlist';
  id: string;
  title: string;
  creator: string;
  trackCount: number;
  thumbnail?: string;
  url: string;
}

export interface StreamingAlbum {
  type: 'album';
  id: string;
  title: string;
  artist: string;
  trackCount: number;
  year?: number;
  thumbnail?: string;
  url: string;
}

export interface StreamingChannel {
  type: 'channel';
  id: string;
  name: string;
  subscriberCount?: number;
  thumbnail?: string;
  url: string;
}

export type SearchResult =
  | StreamingTrack
  | StreamingPlaylist
  | StreamingAlbum
  | StreamingChannel;

export interface ProviderSettings {
  enabled: boolean;
  credentials?: string | null;
}

export interface ProviderConfig {
  youtube: ProviderSettings;
  soundcloud: ProviderSettings;
  bandcamp: ProviderSettings;
  vk: ProviderSettings;
}

export interface AudioConfig {
  // Empty for now as per backend
}

export interface NetworkConfig {
  timeout_seconds: number;
  chunk_size: number;
  buffer_size: number;
}

export interface AppConfig {
  providers: ProviderConfig;
  audio: AudioConfig;
  network: NetworkConfig;
}
