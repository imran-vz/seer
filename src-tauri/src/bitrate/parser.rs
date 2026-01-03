//! FFprobe parsing and bitrate statistics calculation
//!
//! This module handles:
//! - Parsing frame data from ffprobe output
//! - Aggregating frame data into time intervals
//! - Calculating bitrate statistics

use log::{debug, error, info};
use std::io::Read;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use crate::types::{BitrateDataPoint, BitrateStatistics, PeakInterval, StreamInfo, StreamType};

/// Parse ffprobe frame data for a specific stream
///
/// Returns a vector of (timestamp, size, frame_type) tuples
pub fn parse_ffprobe_frames(
    path: &str,
    stream_index: i32,
) -> Result<Vec<(f64, u64, Option<String>)>, String> {
    // Spawn ffprobe process - use multiple timestamp fields for compatibility
    let mut child = Command::new("ffprobe")
        .arg("-v")
        .arg("error") // Show errors instead of quiet
        .arg("-select_streams")
        .arg(stream_index.to_string())
        .arg("-show_frames")
        .arg("-show_entries")
        // Include multiple timestamp fields - some formats use different ones
        .arg("frame=best_effort_timestamp_time,pkt_pts_time,pts_time,pkt_dts_time,pkt_size,pict_type")
        .arg("-of")
        .arg("json")
        .arg(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn ffprobe: {}", e))?;

    // Take ownership of stdout/stderr handles to read in separate threads
    // This prevents pipe buffer deadlock when ffprobe produces large output
    let stdout_handle = child.stdout.take();
    let stderr_handle = child.stderr.take();

    // Spawn thread to read stdout (prevents pipe buffer from filling up and blocking ffprobe)
    let stdout_thread = thread::spawn(move || {
        let mut stdout = Vec::new();
        if let Some(mut out) = stdout_handle {
            out.read_to_end(&mut stdout).ok();
        }
        stdout
    });

    // Spawn thread to read stderr
    let stderr_thread = thread::spawn(move || {
        let mut stderr = Vec::new();
        if let Some(mut err) = stderr_handle {
            err.read_to_end(&mut stderr).ok();
        }
        stderr
    });

    // Wait for completion with timeout (5 minutes max)
    let timeout = Duration::from_secs(300);
    let start = std::time::Instant::now();

    let status = loop {
        if start.elapsed() > timeout {
            let _ = child.kill();
            return Err(format!(
                "ffprobe timed out after {} seconds",
                timeout.as_secs()
            ));
        }

        match child.try_wait() {
            Ok(Some(status)) => {
                break status;
            }
            Ok(None) => {
                // Still running, sleep briefly
                thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                let _ = child.kill();
                return Err(format!("Failed to wait for ffprobe: {}", e));
            }
        }
    };

    // Collect output from threads
    let stdout = stdout_thread
        .join()
        .map_err(|_| "Failed to join stdout thread")?;
    let stderr = stderr_thread
        .join()
        .map_err(|_| "Failed to join stderr thread")?;

    if !status.success() {
        let err_msg = String::from_utf8_lossy(&stderr);
        return Err(format!("ffprobe failed: {}", err_msg));
    }

    // Log any warnings from stderr
    if !stderr.is_empty() {
        let err_msg = String::from_utf8_lossy(&stderr);
        debug!("ffprobe stderr for stream {}: {}", stream_index, err_msg);
    }

    let json_str = String::from_utf8_lossy(&stdout);

    // Check if output is empty
    if json_str.trim().is_empty() {
        return Err(format!(
            "ffprobe returned empty output for stream {}",
            stream_index
        ));
    }

    let parsed: serde_json::Value =
        serde_json::from_str(&json_str).map_err(|e| format!("Failed to parse JSON: {}", e))?;

    let frames = parsed["frames"].as_array().ok_or_else(|| {
        format!(
            "No frames array found in ffprobe output for stream {}",
            stream_index
        )
    })?;

    if frames.is_empty() {
        return Err(format!("No frames found for stream {}", stream_index));
    }

    let mut result: Vec<(f64, u64, Option<String>)> = Vec::new();
    let mut skipped = 0;

    for (idx, frame) in frames.iter().enumerate() {
        // Try multiple timestamp fields (different formats use different ones)
        let timestamp = frame["best_effort_timestamp_time"]
            .as_str()
            .and_then(|s| s.parse::<f64>().ok())
            .or_else(|| {
                frame["pkt_pts_time"]
                    .as_str()
                    .and_then(|s| s.parse::<f64>().ok())
            })
            .or_else(|| {
                frame["pts_time"]
                    .as_str()
                    .and_then(|s| s.parse::<f64>().ok())
            })
            .or_else(|| {
                frame["pkt_dts_time"]
                    .as_str()
                    .and_then(|s| s.parse::<f64>().ok())
            });

        let size = frame["pkt_size"]
            .as_str()
            .and_then(|s| s.parse::<u64>().ok())
            .or_else(|| frame["pkt_size"].as_u64());

        // If no timestamp but we have size, use fallback (frame index based)
        let final_timestamp = if let Some(ts) = timestamp {
            ts
        } else if result.is_empty() {
            // First frame with no timestamp, assume 0
            0.0
        } else {
            // Use frame index to estimate timestamp
            // This is a fallback for formats that don't provide timestamps
            // Assume ~30fps if we can't determine actual rate
            let estimated_fps = if result.len() > 1 {
                // Calculate fps from existing data
                let last_ts = result.last().unwrap().0;
                if last_ts > 0.0 {
                    result.len() as f64 / last_ts
                } else {
                    30.0
                }
            } else {
                30.0
            };
            idx as f64 / estimated_fps
        };

        // Skip frames with invalid size
        if size.is_none() || size == Some(0) {
            skipped += 1;
            continue;
        }

        let frame_type = frame["pict_type"].as_str().map(|s| s.to_string());
        result.push((final_timestamp, size.unwrap(), frame_type));
    }

    if skipped > 0 {
        debug!(
            "Skipped {} frames with missing data for stream {}",
            skipped, stream_index
        );
    }

    if result.is_empty() {
        error!("All frames had invalid data for stream {}", stream_index);
        return Err(format!(
            "All frames had invalid data for stream {}",
            stream_index
        ));
    }

    info!(
        "Successfully parsed {} frames for stream {}",
        result.len(),
        stream_index
    );
    Ok(result)
}

/// Aggregate frames into time intervals for bitrate calculation
pub fn aggregate_bitrate_intervals(
    frames: Vec<(f64, u64, Option<String>)>,
    interval_seconds: f64,
    duration: f64,
) -> Vec<BitrateDataPoint> {
    if frames.is_empty() {
        return Vec::new();
    }

    let num_intervals = (duration / interval_seconds).ceil() as usize;
    let mut intervals: Vec<(u64, usize, Option<String>)> = vec![(0, 0, None); num_intervals];

    for (timestamp, size, frame_type) in frames {
        let interval_idx = (timestamp / interval_seconds).floor() as usize;
        if interval_idx < num_intervals {
            intervals[interval_idx].0 += size;
            intervals[interval_idx].1 += 1;
            if intervals[interval_idx].2.is_none() {
                intervals[interval_idx].2 = frame_type;
            }
        }
    }

    intervals
        .into_iter()
        .enumerate()
        .map(|(idx, (total_size, _, frame_type))| {
            let bitrate = ((total_size * 8) as f64 / interval_seconds) as u64;
            BitrateDataPoint {
                timestamp: idx as f64 * interval_seconds,
                bitrate,
                frame_type,
            }
        })
        .collect()
}

/// Calculate statistics from bitrate data points
pub fn calculate_statistics(data_points: &[BitrateDataPoint]) -> BitrateStatistics {
    if data_points.is_empty() {
        return BitrateStatistics {
            min_bitrate: 0,
            max_bitrate: 0,
            avg_bitrate: 0,
            median_bitrate: 0,
            std_deviation: 0.0,
            peak_intervals: Vec::new(),
            total_frames: 0,
        };
    }

    let bitrates: Vec<u64> = data_points.iter().map(|d| d.bitrate).collect();
    let min_bitrate = *bitrates.iter().min().unwrap_or(&0);
    let max_bitrate = *bitrates.iter().max().unwrap_or(&0);
    let avg_bitrate = bitrates.iter().sum::<u64>() / bitrates.len() as u64;

    let mut sorted_bitrates = bitrates.clone();
    sorted_bitrates.sort_unstable();
    let median_bitrate = if sorted_bitrates.len() % 2 == 0 {
        (sorted_bitrates[sorted_bitrates.len() / 2 - 1]
            + sorted_bitrates[sorted_bitrates.len() / 2])
            / 2
    } else {
        sorted_bitrates[sorted_bitrates.len() / 2]
    };

    let variance = bitrates
        .iter()
        .map(|&b| {
            let diff = b as f64 - avg_bitrate as f64;
            diff * diff
        })
        .sum::<f64>()
        / bitrates.len() as f64;
    let std_deviation = variance.sqrt();

    // Detect peaks (bitrate > 1.5x average for > 5 seconds)
    let peak_threshold = (avg_bitrate as f64 * 1.5) as u64;
    let mut peak_intervals = Vec::new();
    let mut in_peak = false;
    let mut peak_start = 0.0;
    let mut peak_bitrate = 0u64;

    for point in data_points {
        if point.bitrate > peak_threshold {
            if !in_peak {
                in_peak = true;
                peak_start = point.timestamp;
                peak_bitrate = point.bitrate;
            } else {
                peak_bitrate = peak_bitrate.max(point.bitrate);
            }
        } else if in_peak {
            let duration = point.timestamp - peak_start;
            if duration > 5.0 {
                peak_intervals.push(PeakInterval {
                    start_time: peak_start,
                    end_time: point.timestamp,
                    peak_bitrate,
                    duration,
                });
            }
            in_peak = false;
        }
    }

    BitrateStatistics {
        min_bitrate,
        max_bitrate,
        avg_bitrate,
        median_bitrate,
        std_deviation,
        peak_intervals,
        total_frames: data_points.len(),
    }
}

/// Sort streams so audio comes first (faster to process), then video
pub fn sort_streams_audio_first(streams: &mut [&StreamInfo]) {
    streams.sort_by(|a, b| {
        let priority_a = match a.stream_type {
            StreamType::Audio => 0,
            StreamType::Video => 1,
            _ => 2,
        };
        let priority_b = match b.stream_type {
            StreamType::Audio => 0,
            StreamType::Video => 1,
            _ => 2,
        };
        priority_a.cmp(&priority_b).then(a.index.cmp(&b.index))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aggregate_empty_frames() {
        let frames: Vec<(f64, u64, Option<String>)> = vec![];
        let result = aggregate_bitrate_intervals(frames, 1.0, 10.0);
        assert!(result.is_empty());
    }

    #[test]
    fn test_calculate_statistics_empty() {
        let data: Vec<BitrateDataPoint> = vec![];
        let stats = calculate_statistics(&data);
        assert_eq!(stats.min_bitrate, 0);
        assert_eq!(stats.max_bitrate, 0);
        assert_eq!(stats.avg_bitrate, 0);
    }

    #[test]
    fn test_calculate_statistics() {
        let data = vec![
            BitrateDataPoint {
                timestamp: 0.0,
                bitrate: 1000,
                frame_type: None,
            },
            BitrateDataPoint {
                timestamp: 1.0,
                bitrate: 2000,
                frame_type: None,
            },
            BitrateDataPoint {
                timestamp: 2.0,
                bitrate: 3000,
                frame_type: None,
            },
        ];
        let stats = calculate_statistics(&data);
        assert_eq!(stats.min_bitrate, 1000);
        assert_eq!(stats.max_bitrate, 3000);
        assert_eq!(stats.avg_bitrate, 2000);
        assert_eq!(stats.median_bitrate, 2000);
        assert_eq!(stats.total_frames, 3);
    }
}
