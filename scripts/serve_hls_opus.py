#!/usr/bin/env python3
"""Serve a continuous Ogg/Opus track as HLS, like SoundCloud's old Opus streams.

Run without arguments for the bundled sample, or pass a local Ogg/Opus file.
Segments preserve the original Ogg pages; only the first contains Opus headers.
No third-party Python packages or transcoding are required.
"""

import argparse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import math
from pathlib import Path
import struct
from urllib.parse import urlsplit


def segment_track(data, target_duration):
    """Split one Ogg Opus logical stream at page boundaries using granule times."""
    if data[:4] != b"OggS":
        raise ValueError("Input must be an Ogg/Opus file (OpusHead in the first page)")
    segments = []
    offset = start = 0
    previous_time = current_time = 0.0
    serial = None
    pre_skip = None
    ended = False
    while offset < len(data):
        if ended:
            raise ValueError("Chained Ogg streams are not supported")
        if len(data) - offset < 27 or data[offset : offset + 5] != b"OggS\x00":
            raise ValueError(f"Invalid Ogg page at byte {offset}")
        count = data[offset + 26]
        header_end = offset + 27 + count
        if header_end > len(data):
            raise ValueError("Truncated Ogg page header")
        end = header_end + sum(data[offset + 27 : header_end])
        if end > len(data):
            raise ValueError("Truncated Ogg page body")
        page_serial = struct.unpack_from("<I", data, offset + 14)[0]
        if serial is None:
            serial = page_serial
            if end - header_end < 19 or data[header_end : header_end + 8] != b"OpusHead":
                raise ValueError("Missing OpusHead packet")
            pre_skip = struct.unpack_from("<H", data, header_end + 10)[0]
        elif page_serial != serial:
            raise ValueError("Multiplexed Ogg streams are not supported")
        granule = struct.unpack_from("<Q", data, offset + 6)[0]
        if granule != 0xFFFFFFFFFFFFFFFF:
            # Opus Ogg granule positions always use a 48 kHz clock.
            current_time = max(0.0, (granule - pre_skip) / 48000)
        ended = bool(data[offset + 5] & 4)
        if current_time - previous_time >= target_duration or ended:
            duration = current_time - previous_time
            if duration <= 0:
                raise ValueError("Ogg stream has an empty or invalid audio duration")
            segments.append((duration, data[start:end]))
            start, previous_time = end, current_time
        offset = end
    if not ended or start != len(data) or not segments:
        raise ValueError("Input must be a complete Ogg/Opus track with an end page")
    return segments


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "track", nargs="?", type=Path,
        default=Path(__file__).resolve().parents[1]
        / "vendor/symphonia-adapters/assets/sample.opus",
    )
    parser.add_argument("--port", type=int, default=8765)
    parser.add_argument("--segment-duration", type=float, default=2.0)
    args = parser.parse_args()
    if not 0 < args.segment_duration < float("inf"):
        parser.error("--segment-duration must be a positive finite number")
    try:
        segments = segment_track(args.track.read_bytes(), args.segment_duration)
    except (OSError, ValueError, struct.error) as error:
        parser.error(str(error))

    playlist = [
        "#EXTM3U", "#EXT-X-VERSION:3", "#EXT-X-PLAYLIST-TYPE:VOD",
        f"#EXT-X-TARGETDURATION:{math.ceil(max(d for d, _ in segments))}",
        "#EXT-X-MEDIA-SEQUENCE:0",
    ]
    routes = {}
    for index, (duration, payload) in enumerate(segments):
        name = f"segment{index:04d}.opus"
        playlist.extend([f"#EXTINF:{duration:.6f},", name])
        routes[f"/{name}"] = ("audio/ogg; codecs=opus", payload)
    playlist.append("#EXT-X-ENDLIST")
    manifest = ("application/vnd.apple.mpegurl", ("\n".join(playlist) + "\n").encode())
    routes["/"] = routes["/playlist.m3u8"] = manifest

    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            route = routes.get(urlsplit(self.path).path)
            if route is None:
                self.send_error(404)
                return
            content_type, payload = route
            self.send_response(200)
            self.send_header("Content-Type", content_type)
            self.send_header("Content-Length", str(len(payload)))
            self.send_header("Access-Control-Allow-Origin", "*")
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            if self.command != "HEAD":
                self.wfile.write(payload)

        do_HEAD = do_GET

    with ThreadingHTTPServer(("127.0.0.1", args.port), Handler) as server:
        print(f"Track: {args.track}", flush=True)
        print(f"Ogg/Opus: {len(segments)} segments, {sum(d for d, _ in segments):.3f}s", flush=True)
        print(f"HLS URL: http://127.0.0.1:{server.server_port}/playlist.m3u8", flush=True)
        try:
            server.serve_forever()
        except KeyboardInterrupt:
            pass


if __name__ == "__main__":
    main()
