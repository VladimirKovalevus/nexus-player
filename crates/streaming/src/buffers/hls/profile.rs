use super::{lock_recover, HlsBuffer};

impl HlsBuffer {
    pub fn profile(&self) {
        use std::fmt::Write as _;
        let mut out = String::new();
        let inner = lock_recover(&self.inner);

        let total_segments = inner.segments.len();
        let filled_segments = inner.segments.iter().filter(|s| s.is_some()).count();

        let total_duration: f64 = inner.durations.iter().sum();
        let downloaded = inner
            .segments
            .iter()
            .filter_map(|s| s.as_ref())
            .map(|s| s.data.len() as u64)
            .sum::<u64>();

        let _ = writeln!(
            out,
            "╔═══════════════════════════════════════════════════════════════╗"
        );
        let _ = writeln!(
            out,
            "║               HLS BUFFER PROFILE                              ║"
        );
        let _ = writeln!(
            out,
            "╠═══════════════════════════════════════════════════════════════╣"
        );
        let _ = writeln!(
            out,
            "║ Total segments:  {:<10} │ Filled: {:<10}              ║",
            total_segments, filled_segments
        );
        let _ = writeln!(
            out,
            "║ Duration:        {:<10.2}s│ Bitrate: {:<10} kbps     ║",
            total_duration,
            inner.bitrate / 1000
        );
        let _ = writeln!(
            out,
            "║ Downloaded:      {:<10} bytes                             ║",
            downloaded
        );
        let _ = writeln!(
            out,
            "║ Progress:        {:.2}%                                        ║",
            if total_segments > 0 {
                (filled_segments as f64 / total_segments as f64) * 100.0
            } else {
                0.0
            }
        );
        let _ = writeln!(
            out,
            "║ Has init seg:    {:<10}                                   ║",
            if inner.has_init_segment { "Yes" } else { "No" }
        );
        let _ = writeln!(
            out,
            "╠═══════════════════════════════════════════════════════════════╣"
        );
        let _ = writeln!(
            out,
            "║ Segment Map (■ = filled, □ = empty):                         ║"
        );
        let _ = writeln!(
            out,
            "╠═══════════════════════════════════════════════════════════════╣"
        );

        const SEGMENTS_PER_ROW: usize = 50;

        for (row_idx, row) in inner.segments.chunks(SEGMENTS_PER_ROW).enumerate() {
            let start_idx = row_idx * SEGMENTS_PER_ROW;
            let _ = write!(
                out,
                "║ {:04}-{:04} │ ",
                start_idx,
                start_idx + row.len() - 1
            );

            for segment_opt in row {
                let symbol = match segment_opt {
                    None => '□',
                    Some(_) => '■',
                };
                let _ = write!(out, "{}", symbol);
            }

            let padding = SEGMENTS_PER_ROW.saturating_sub(row.len());
            for _ in 0..padding {
                let _ = write!(out, " ");
            }

            let _ = writeln!(out, " ║");
        }

        let _ = writeln!(
            out,
            "╠═══════════════════════════════════════════════════════════════╣"
        );
        let _ = writeln!(
            out,
            "║ Detailed Segment Info:                                       ║"
        );
        let _ = writeln!(
            out,
            "╠═════════╦══════════════╦═══════════════╦═════════════════════╣"
        );
        let _ = writeln!(
            out,
            "║  Index  ║    Status    ║  Size (bytes) ║  Duration (s)       ║"
        );
        let _ = writeln!(
            out,
            "╠═════════╬══════════════╬═══════════════╬═════════════════════╣"
        );

        for (idx, segment_opt) in inner.segments.iter().enumerate() {
            let duration = if idx == 0 && inner.has_init_segment {
                0.0
            } else {
                let duration_idx = if inner.has_init_segment { idx - 1 } else { idx };
                inner.durations.get(duration_idx).copied().unwrap_or(0.0)
            };

            match segment_opt {
                None => {
                    let _ = writeln!(
                        out,
                        "║ {:>7} ║ Empty        ║ {:>13} ║ {:>7.2}             ║",
                        idx, 0, duration
                    );
                }
                Some(segment) => {
                    let status = if idx == 0 && inner.has_init_segment {
                        "Init"
                    } else {
                        "Filled"
                    };
                    let _ = writeln!(
                        out,
                        "║ {:>7} ║ {:<12} ║ {:>13} ║ {:>7.2}             ║",
                        idx,
                        status,
                        segment.data.len(),
                        duration
                    );
                }
            }
        }

        let _ = writeln!(
            out,
            "╚═════════╩══════════════╩═══════════════╩═════════════════════╝"
        );
        tracing::debug!("{out}");
    }
}
