export interface TrackSourceDto {
  url: string;
  system: string;
}

export interface TrackDto {
  id: number;
  title: string;
  artist: string | null;
  album: string | null;
  durationSecs: number | null;
  artworkUrl: string | null;
  playCount: number;
  state: string;
  source?: TrackSourceDto | null;
}

export interface PlaylistDto {
  id: number;
  title: string;
  description: string | null;
  creator: string | null;
  thumbnail: string | null;
  trackCount: number;
  createdAt: number;
  updatedAt: number;
  source?: TrackSourceDto | null;
}

export type PlaylistItemDto =
  | { itemType: 'track'; itemId: number; track: TrackDto; position: number }
  | { itemType: 'playlist'; itemId: number; playlist: PlaylistDto; position: number };

export interface PlaylistWithItemsDto {
  playlist: PlaylistDto;
  items: PlaylistItemDto[];
}

export interface BufferedRange {
  start: number;
  end: number;
}

export interface Waveform {
  data: number[];
  startTime: number;
  endTime: number;
  byteStart?: number | null;
  byteEnd?: number | null;
}

export interface AudioState {
  state: 'playing' | 'paused' | 'stopped' | 'buffering' | 'loading';
  positionSecs: number;
  durationSecs: number;
  volume: number;
  currentTrack: TrackDto | null;
  bufferedRanges: BufferedRange[];
  waveforms: Waveform[];
}

export interface ContainerMetaDto {
  kind: 'album' | 'playlist';
  id: string;
  system: string;
  title: string;
  artist: string;
  trackCount: number;
  thumbnail: string | null;
}

export type QueueEntryDto =
  | { entryType: 'track'; track: TrackDto }
  | { entryType: 'container'; meta: ContainerMetaDto; tracks: TrackDto[] | null; playIndex: number };

export interface CurrentPositionDto {
  itemIndex: number;
  trackWithinContainer: number | null;
}

export interface SearchContextDto {
  system: string;
  query: string;
}

export interface QueueState {
  entries: QueueEntryDto[];
  currentPosition: CurrentPositionDto | null;
  repeatMode: 'off' | 'all' | 'one';
  shuffleEnabled: boolean;
  searchContext: SearchContextDto | null;
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
  waveformEnabled: boolean;
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
