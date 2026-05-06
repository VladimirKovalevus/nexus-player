export interface EqSettingsDto {
  enabled: boolean;
  gains: number[];
  presetName: string | null;
}

export interface EqPresetDto {
  name: string;
  gains: number[];
}

export interface TrackSourceDto {
  url: string;
  system: string;
}

export interface TrackStateDto {
  type: 'saved' | 'streaming' | 'cached';
}

export interface TrackDto {
  id: number;
  title: string;
  artist: string | null;
  album: string | null;
  durationSecs: number | null;
  artworkUrl: string | null;
  playCount: number;
  liked: boolean;
  state: TrackStateDto;
  source?: TrackSourceDto | null;
  unavailable?: boolean;
}

export interface CachedAlbumDto {
  title: string;
  artist: string | null;
  artworkUrl: string | null;
  cachedTrackCount: number;
}

export interface MusicSourceDto {
  id: number;
  path: string;
  isDefault: boolean;
}

export interface LocalScanResult {
  found: number;
  added: number;
  updated: number;
  skipped: number;
  unavailable: number;
  errors: number;
}

export interface AndroidMediaEntry {
  sourceKey: string;
  path: string;
  size: number | null;
  modifiedAt: number | null;
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
  | { id: number; itemType: 'track'; itemId: number; track: TrackDto; position: number }
  | { id: number; itemType: 'playlist'; itemId: number; playlist: PlaylistDto; position: number };

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
  fft: number[];
  pcm: number[];
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

export interface LibraryPageDto {
  currentPage: number;
  totalPages: number;
  pageSize: number;
  totalTracks: number;
  trackIndexInPage: number;
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
  id?: number;
  title: string;
  artist: string;
  durationSecs?: number;
  thumbnail?: string;
  source?: TrackSourceDto;
  state?: { type: string };
  liked: boolean;
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
  soundcloud: ProviderSettings;
  bandcamp: ProviderSettings;
}

export interface AudioConfig {
  waveformEnabled: boolean;
  memLimit: number;
  fadeInEnabled: boolean;
  fadeOutEnabled: boolean;
  fadeInMs: number;
  fadeOutMs: number;
}

export interface NetworkConfig {
  timeoutSeconds: number;
  chunkSize: number;
  bufferSize: number;
}

export interface AppConfig {
  providers: ProviderConfig;
  audio: AudioConfig;
  network: NetworkConfig;
}

export interface PlayHistoryEntryDto {
  id: number;
  title: string;
  artist: string | null;
  source: TrackSourceDto;
  artworkUrl: string | null;
  playedAt: number;
}

export interface TaskInfoDto {
  trackId: number;
  url: string;
  state: string;
  sizeHuman: string;
  byteSize: number;
  progress: number;
  meta: StreamingTrack | null;
}

export type SyncType = 'full' | 'diff';
