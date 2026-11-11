use crate::{
    error::StreamingError,
    hls::segment::{
        HlsPlaylist, HlsPlaylistType, HlsSegment, MasterPlaylist, PlaylistType, StreamVariant,
    },
};
pub fn parse_m3u8(content: &str, base_url: &str) -> Result<HlsPlaylist, StreamingError> {
    match parse_playlist(content, base_url)? {
        HlsPlaylistType::Media(media) => Ok(media),
        HlsPlaylistType::Master(_) => Err(StreamingError::HlsParseError(
            "Expected media playlist, got master".to_string(),
        )),
    }
}
pub fn select_variant(master: &MasterPlaylist, bandwidth_bps: u64) -> Option<&StreamVariant> {
    master.select_variant(bandwidth_bps)
}

pub fn parse_media_playlist(content: &str, url: &str) -> Result<HlsPlaylist, StreamingError> {
    let lines: Vec<&str> = content.lines().collect();

    if lines.is_empty() || !lines[0].starts_with("#EXTM3U") {
        return Err(StreamingError::HlsParseError(
            "Invalid M3U8: missing #EXTM3U header".to_string(),
        ));
    }

    let mut playlist = HlsPlaylist::new();
    let mut current_duration = 0.0;
    let mut sequence = 0u64;
    let mut byte_offset = 0u64;

    let mut next_range_offset = 0u64;
    let mut current_range_length = None;
    let mut current_range_offset = None;

    for line in lines.iter().skip(1) {
        let line = line.trim();

        if line.is_empty() {
            continue;
        }

        if let Some(attrs) = line.strip_prefix("#EXT-X-MAP:") {
            if let Some(uri) = extract_attribute(attrs, "URI") {
                let full_url = resolve_url(url, &uri);
                let mut range_length = None;
                let mut range_offset = None;

                if let Some(br) = extract_attribute(attrs, "BYTERANGE") {
                    if let Some((len, off)) = parse_byte_range(&br, 0) {
                        range_length = Some(len);
                        range_offset = Some(off);
                    }
                }

                playlist.init_segment = Some(crate::hls::segment::InitSegment {
                    url: full_url,
                    range_length,
                    range_offset,
                });
            }
        } else if line.starts_with("#EXT-X-BYTERANGE:") {
            let br_val = line.trim_start_matches("#EXT-X-BYTERANGE:");
            if let Some((len, off)) = parse_byte_range(br_val, next_range_offset) {
                current_range_length = Some(len);
                current_range_offset = Some(off);
                next_range_offset = off + len;
            }
        } else if line.starts_with("#EXTINF:") {
            let duration_str = line
                .trim_start_matches("#EXTINF:")
                .split(',')
                .next()
                .unwrap_or("0");

            current_duration = duration_str.parse().unwrap_or(0.0);
        } else if line.starts_with("#EXT-X-TARGETDURATION:") {
            let duration_str = line.trim_start_matches("#EXT-X-TARGETDURATION:");
            playlist.target_duration = duration_str.parse().unwrap_or(10);
        } else if line.starts_with("#EXT-X-PLAYLIST-TYPE:") {
            let ptype = line.trim_start_matches("#EXT-X-PLAYLIST-TYPE:");
            playlist.playlist_type = match ptype {
                "VOD" => PlaylistType::Vod,
                "EVENT" => PlaylistType::Live,
                _ => PlaylistType::Vod,
            };
        } else if line.starts_with("#EXT-X-ENDLIST") {
            playlist.playlist_type = PlaylistType::Vod;
        } else if !line.starts_with('#') {
            let url = resolve_url(url, line);

            let segment = HlsSegment {
                url: url.clone(),
                duration: current_duration,
                byte_offset,
                byte_size: current_range_length,
                sequence,
                range_length: current_range_length,
                range_offset: current_range_offset,
            };

            playlist.segments.push(segment);

            sequence += 1;
            // Only update fake byte_offset if we don't have real byte ranges
            match current_range_length {
                Some(length) => byte_offset += length,
                None => byte_offset += (current_duration * 20_000.0) as u64,
            }

            current_duration = 0.0;
            current_range_length = None;
            current_range_offset = None;
        }
    }

    Ok(playlist)
}

fn parse_byte_range(br: &str, last_offset: u64) -> Option<(u64, u64)> {
    let br = br.trim().trim_matches('"');
    if let Some(at_idx) = br.find('@') {
        let len_str = &br[..at_idx];
        let off_str = &br[at_idx + 1..];
        if let (Ok(len), Ok(off)) = (len_str.parse::<u64>(), off_str.parse::<u64>()) {
            return Some((len, off));
        }
    } else if let Ok(len) = br.parse::<u64>() {
        return Some((len, last_offset));
    }
    None
}
#[cfg(test)]
fn extract_uri(line: &str, attribute: &str) -> Option<String> {
    let pattern = format!("{}=\"", attribute);

    if let Some(start_idx) = line.find(&pattern) {
        let start = start_idx + pattern.len();

        if let Some(end_idx) = line[start..].find('"') {
            let uri = &line[start..start + end_idx];
            return Some(uri.to_string());
        }
    }

    None
}
fn resolve_url(base: &str, relative: &str) -> String {
    if relative.starts_with("http://") || relative.starts_with("https://") {
        return relative.to_string();
    }

    if relative.starts_with('/') {
        // Resolve against host
        if let Some(proto_end) = base.find("://") {
            let rest = &base[proto_end + 3..];
            if let Some(host_end) = rest.find('/') {
                let host = &base[..proto_end + 3 + host_end];
                return format!("{}{}", host, relative);
            } else {
                return format!("{}{}", base, relative);
            }
        }
    }

    if base.contains('/') {
        if let Some(last_slash) = base.rfind('/') {
            let base_dir = &base[..last_slash];
            return format!("{}/{}", base_dir, relative);
        }
    }

    format!("{}/{}", base, relative)
}

pub fn parse_playlist(content: &str, base_url: &str) -> Result<HlsPlaylistType, StreamingError> {
    let lines: Vec<&str> = content.lines().collect();

    if lines.is_empty() || !lines[0].starts_with("#EXTM3U") {
        return Err(StreamingError::HlsParseError(
            "Invalid M3U8: missing #EXTM3U header".to_string(),
        ));
    }

    let is_master = content.contains("#EXT-X-STREAM-INF");

    if is_master {
        parse_master_playlist(content, base_url).map(HlsPlaylistType::Master)
    } else {
        parse_media_playlist(content, base_url).map(HlsPlaylistType::Media)
    }
}
fn parse_master_playlist(content: &str, base_url: &str) -> Result<MasterPlaylist, StreamingError> {
    let lines: Vec<&str> = content.lines().collect();
    let mut variants = Vec::new();

    let mut current_bandwidth = 0u64;
    let mut current_codecs: Option<String> = None;
    let mut current_resolution: Option<String> = None;

    for line in lines.iter().skip(1) {
        let line = line.trim();

        if line.is_empty() {
            continue;
        }

        if line.starts_with("#EXT-X-STREAM-INF:") {
            // Parse attributes
            let attrs = line.trim_start_matches("#EXT-X-STREAM-INF:");

            if let Some(bw) = extract_attribute(attrs, "BANDWIDTH") {
                current_bandwidth = bw.parse().unwrap_or(0);
            }

            current_codecs = extract_attribute(attrs, "CODECS");

            current_resolution = extract_attribute(attrs, "RESOLUTION");
        } else if !line.starts_with('#') {
            let uri = resolve_url(base_url, line);

            variants.push(StreamVariant {
                uri,
                bandwidth: current_bandwidth,
                codecs: current_codecs.clone(),
                resolution: current_resolution.clone(),
            });

            // Reset
            current_bandwidth = 0;
            current_codecs = None;
            current_resolution = None;
        }
    }

    if variants.is_empty() {
        return Err(StreamingError::HlsParseError(
            "No variants found in master playlist".to_string(),
        ));
    }

    Ok(MasterPlaylist { variants })
}
fn extract_attribute(attrs: &str, name: &str) -> Option<String> {
    let pattern = format!("{}=", name);

    if let Some(start_idx) = attrs.find(&pattern) {
        let start = start_idx + pattern.len();
        let remaining = &attrs[start..];

        if let Some(quoted) = remaining.strip_prefix('"') {
            if let Some(end_idx) = quoted.find('"') {
                return Some(quoted[..end_idx].to_string());
            }
        } else {
            let end_idx = remaining.find(',').unwrap_or(remaining.len());
            return Some(remaining[..end_idx].to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_playlist() {
        let content = r#"#EXTM3U
#EXT-X-VERSION:7
#EXT-X-TARGETDURATION:10
#EXTINF:10.0,
https://example.com/seg0.m4s
#EXTINF:10.0,
https://example.com/seg1.m4s
#EXT-X-ENDLIST
"#;

        let playlist = parse_m3u8(content, "https://example.com/").unwrap();

        assert_eq!(playlist.segments.len(), 2);
        assert_eq!(playlist.segments[0].duration, 10.0);
        assert_eq!(playlist.segments[0].url, "https://example.com/seg0.m4s");
        assert_eq!(playlist.playlist_type, PlaylistType::Vod);
        assert_eq!(playlist.target_duration, 10);
    }

    #[test]
    fn test_parse_with_init_segment() {
        let content = r#"#EXTM3U
#EXT-X-VERSION:7
#EXT-X-MAP:URI="init.mp4"
#EXTINF:10.0,
seg0.m4s
#EXT-X-ENDLIST
"#;

        let playlist = parse_m3u8(content, "https://example.com/media/").unwrap();

        assert_eq!(
            playlist.init_segment.as_ref().map(|s| s.url.clone()),
            Some("https://example.com/media/init.mp4".to_string())
        );
        assert_eq!(
            playlist.segments[0].url,
            "https://example.com/media/seg0.m4s"
        );
    }

    #[test]
    fn test_parse_relative_urls() {
        let content = r#"#EXTM3U
#EXTINF:10.0,
media/seg0.m4s
#EXTINF:10.0,
media/seg1.m4s
"#;

        let playlist = parse_m3u8(content, "https://example.com/playlist.m3u8").unwrap();

        assert_eq!(
            playlist.segments[0].url,
            "https://example.com/media/seg0.m4s"
        );
        assert_eq!(
            playlist.segments[1].url,
            "https://example.com/media/seg1.m4s"
        );
    }

    #[test]
    fn test_extract_uri() {
        let line = r#"#EXT-X-MAP:URI="init.mp4""#;
        let uri = extract_uri(line, "URI").unwrap();
        assert_eq!(uri, "init.mp4");

        let line2 = r#"#EXT-X-MAP:URI="path/to/init.mp4",BYTERANGE="1024@0""#;
        let uri2 = extract_uri(line2, "URI").unwrap();
        assert_eq!(uri2, "path/to/init.mp4");
    }

    #[test]
    fn test_resolve_url() {
        let result = resolve_url("https://example.com/", "https://other.com/file.m4s");
        assert_eq!(result, "https://other.com/file.m4s");

        let result = resolve_url("https://example.com/media/", "seg0.m4s");
        assert_eq!(result, "https://example.com/media/seg0.m4s");

        let result = resolve_url("https://example.com/media/playlist.m3u8", "seg0.m4s");
        assert_eq!(result, "https://example.com/media/seg0.m4s");
    }

    #[test]
    fn test_invalid_playlist() {
        let content = "Not a valid M3U8";
        let result = parse_m3u8(content, "https://example.com/");
        assert!(result.is_err());
    }

    #[test]
    fn test_total_duration() {
        let content = r#"#EXTM3U
#EXTINF:10.5,
seg0.m4s
#EXTINF:8.3,
seg1.m4s
#EXTINF:12.1,
seg2.m4s
"#;

        let playlist = parse_m3u8(content, "https://example.com/").unwrap();

        assert_eq!(playlist.total_duration(), 10.5 + 8.3 + 12.1);
    }
}
