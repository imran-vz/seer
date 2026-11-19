//! Bitrate over time, computed from packet sizes. Nothing is decoded.

use crate::probe::StreamKind;
use anyhow::{Context, Result, bail};
use ff::ffi;
use ff::media::Type;
use ffmpeg_the_third as ff;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Series {
    pub stream_index: usize,
    pub kind: StreamKind,
    pub codec: String,
    pub language: Option<String>,
    pub total_bytes: u64,
    /// Bits per second in each interval.
    pub rates: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stats {
    pub average: u64,
    pub median: u64,
    pub min: u64,
    pub max: u64,
    /// Seconds from the start at which the maximum occurs.
    pub peak_at: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Analysis {
    pub duration: f64,
    /// Width of one interval in seconds.
    pub interval: f64,
    /// All streams summed, bits per second per interval.
    pub overall: Vec<f32>,
    pub stats: Stats,
    /// Streams that carried data, largest first.
    pub streams: Vec<Series>,
    pub packets: u64,
}

/// Raised when `cancel` is set while analysing.
#[derive(Debug)]
pub struct Cancelled;

impl std::fmt::Display for Cancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("cancelled")
    }
}
impl std::error::Error for Cancelled {}

/// Reads every packet header in the file and buckets sizes by time.
///
/// `progress` receives a fraction from 0 to 1, at most a few hundred times.
pub fn analyze(
    path: &Path,
    cancel: &AtomicBool,
    mut progress: impl FnMut(f32),
) -> Result<Analysis> {
    let mut input =
        ff::format::input(path).with_context(|| format!("could not read {}", path.display()))?;

    let declared = match input.duration() {
        d if d > 0 => d as f64 / ffi::AV_TIME_BASE as f64,
        _ => 0.0,
    };
    let interval = interval_for(declared);

    struct Track {
        time_base: f64,
        start: f64,
        bytes: Vec<u64>,
        total: u64,
        last_time: f64,
    }
    let mut tracks: Vec<Track> = input
        .streams()
        .map(|s| {
            let time_base = f64::from(s.time_base());
            let start = match s.start_time() {
                ffi::AV_NOPTS_VALUE => 0.0,
                t => t as f64 * time_base,
            };
            Track {
                time_base,
                start,
                bytes: Vec::new(),
                total: 0,
                last_time: 0.0,
            }
        })
        .collect();

    let mut packets = 0u64;
    let mut last_report = -1.0f32;
    let mut end = 0.0f64;

    for item in input.packets() {
        let (stream, packet) = match item {
            Ok(p) => p,
            // A damaged tail should not throw away everything read so far.
            Err(_) if packets > 0 => break,
            Err(e) => return Err(e).context("could not read packets"),
        };
        let Some(track) = tracks.get_mut(stream.index()) else {
            continue;
        };

        let time = match packet.dts().or(packet.pts()) {
            Some(ts) => (ts as f64 * track.time_base - track.start).max(0.0),
            None => track.last_time,
        };
        track.last_time = time;
        end = end.max(time + packet.duration().max(0) as f64 * track.time_base);

        let bucket = (time / interval) as usize;
        if bucket >= track.bytes.len() {
            track.bytes.resize(bucket + 1, 0);
        }
        track.bytes[bucket] += packet.size() as u64;
        track.total += packet.size() as u64;

        packets += 1;
        if packets.is_multiple_of(512) {
            if cancel.load(Ordering::Relaxed) {
                bail!(Cancelled);
            }
            if declared > 0.0 {
                let fraction = (time / declared).clamp(0.0, 1.0) as f32;
                if fraction - last_report >= 0.005 {
                    last_report = fraction;
                    progress(fraction);
                }
            }
        }
    }
    if packets == 0 {
        bail!("no media data found in {}", path.display());
    }

    let duration = if declared > 0.0 { declared } else { end };
    // A short remainder joins the last full interval instead of forming a
    // sliver of its own, which would read as a dip to zero.
    let buckets = ((duration / interval).round() as usize).max(1);
    let last_width = (duration - (buckets - 1) as f64 * interval).max(interval * 0.5);
    let to_rates = |bytes: &[u64]| -> Vec<f32> {
        let mut folded = vec![0u64; buckets];
        for (i, b) in bytes.iter().enumerate() {
            // Packets stamped past the declared end count towards the last interval.
            folded[i.min(buckets - 1)] += b;
        }
        folded
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let width = if i + 1 == buckets {
                    last_width
                } else {
                    interval
                };
                (*b as f64 * 8.0 / width) as f32
            })
            .collect()
    };

    let mut overall_bytes = vec![0u64; buckets];
    let mut streams = Vec::new();
    for (index, track) in tracks.iter().enumerate() {
        if track.total == 0 {
            continue;
        }
        for (i, b) in track.bytes.iter().enumerate() {
            overall_bytes[i.min(buckets - 1)] += b;
        }
        let Some(stream) = input.stream(index) else {
            continue;
        };
        let params = stream.parameters();
        streams.push(Series {
            stream_index: index,
            kind: match params.medium() {
                Type::Video => StreamKind::Video,
                Type::Audio => StreamKind::Audio,
                Type::Subtitle => StreamKind::Subtitle,
                Type::Attachment => StreamKind::Attachment,
                _ => StreamKind::Data,
            },
            codec: params.id().name().to_owned(),
            language: stream
                .metadata()
                .get("language")
                .filter(|l| *l != "und")
                .map(str::to_owned),
            total_bytes: track.total,
            rates: to_rates(&track.bytes),
        });
    }
    streams.sort_by_key(|s| std::cmp::Reverse(s.total_bytes));

    let overall = to_rates(&overall_bytes);
    let total_bytes: u64 = tracks.iter().map(|t| t.total).sum();
    let stats = stats(&overall, total_bytes, duration, interval);
    progress(1.0);

    Ok(Analysis {
        duration,
        interval,
        overall,
        stats,
        streams,
        packets,
    })
}

/// Aim for a few hundred to a few thousand points whatever the length.
fn interval_for(duration: f64) -> f64 {
    match duration {
        d if d <= 0.0 => 1.0,
        d if d < 20.0 => 0.1,
        d if d < 120.0 => 0.25,
        d if d < 600.0 => 0.5,
        d if d < 4.0 * 3600.0 => 1.0,
        _ => 2.0,
    }
}

fn stats(rates: &[f32], total_bytes: u64, duration: f64, interval: f64) -> Stats {
    let mut sorted: Vec<f32> = rates.to_vec();
    sorted.sort_by(f32::total_cmp);
    let (peak_index, max) = rates
        .iter()
        .copied()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap_or((0, 0.0));
    Stats {
        average: if duration > 0.0 {
            (total_bytes as f64 * 8.0 / duration) as u64
        } else {
            0
        },
        median: sorted.get(sorted.len() / 2).copied().unwrap_or(0.0) as u64,
        min: sorted.first().copied().unwrap_or(0.0) as u64,
        max: max as u64,
        peak_at: peak_index as f64 * interval,
    }
}
