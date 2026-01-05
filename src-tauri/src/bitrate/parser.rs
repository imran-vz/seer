//! FFprobe parsing and bitrate statistics calculation
//!
//! This module handles:
//! - Parsing frame data from ffprobe output (accurate but slow)
//! - Parsing packet data from ffprobe output (fast mode)
//! - Sampling mode for very large files (extrapolated bitrate)
//! - Aggregating frame/packet data into time intervals
//! - Calculating bitrate statistics

use log::{debug, error, info, warn};
use std::io::{BufRead, BufReader, Read};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use crate::media::find_command;
use crate::types::{BitrateDataPoint, BitrateStatistics, PeakInterval, StreamInfo, StreamType};

/// File size threshold for sampling mode (5 GB)
/// Files larger than this will use sampling instead of full analysis
pub const SAMPLING_THRESHOLD_BYTES: u64 = 5 * 1024 * 1024 * 1024;

/// Number of sample intervals to analyze for large files
/// We sample at the start, middle, and end for representative data
pub const SAMPLE_COUNT: usize = 10;

/// Duration of each sample interval in seconds
pub const SAMPLE_DURATION_SECS: f64 = 30.0;

/// Parse ffprobe packet data for a specific stream (FAST MODE)
///
/// Uses -show_packets which is significantly faster than -show_frames
/// because it doesn't require decoding. Returns a vector of (timestamp, size, frame_type) tuples.
///
/// For most bitrate analysis purposes, packet-level data is sufficient and
/// provides the same size information with much better performance.
pub fn parse_ffprobe_packets(
    path: &str,
    stream_index: i32,
) -> Result<Vec<(f64, u64, Option<String>)>, String> {
    parse_ffprobe_packets_internal(path, stream_index, None)
}

/// Parse ffprobe packet data with optional read interval for sampling
///
/// When `read_interval` is Some, only reads packets within that time range.
/// Format: "start%+duration" e.g., "10%+30" reads 30 seconds starting at 10s
fn parse_ffprobe_packets_internal(
    path: &str,
    stream_index: i32,
    read_interval: Option<&str>,
) -> Result<Vec<(f64, u64, Option<String>)>, String> {
    let mode_desc = read_interval
        .map(|i| format!("sampled [{}]", i))
        .unwrap_or_else(|| "full".to_string());
    info!(
        "parse_ffprobe_packets ({}): file={}, stream_index={}",
        mode_desc, path, stream_index
    );

    let ffprobe_cmd = find_command("ffprobe").unwrap_or_else(|| "ffprobe".to_string());
    debug!("Using ffprobe command: {}", ffprobe_cmd);

    // Build command with optional read interval
    let mut cmd = Command::new(&ffprobe_cmd);
    cmd.arg("-v")
        .arg("error")
        .arg("-select_streams")
        .arg(stream_index.to_string());

    // Add read interval if sampling
    if let Some(interval) = read_interval {
        cmd.arg("-read_intervals").arg(interval);
    }

    cmd.arg("-show_packets")
        .arg("-show_entries")
        .arg("packet=pts_time,dts_time,size,flags")
        .arg("-of")
        .arg("csv=p=0") // CSV format without packet wrapper, very fast to parse
        .arg(path);

    debug!(
        "Spawning ffprobe for stream {} (packet mode, {})",
        stream_index, mode_desc
    );
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn ffprobe: {}", e))?;

    let stdout_handle = child.stdout.take();
    let stderr_handle = child.stderr.take();

    // Parse stdout in a streaming fashion for better memory efficiency
    let stdout_thread = thread::spawn(move || {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut results: Vec<(f64, u64, Option<String>)> = Vec::new();
            if let Some(out) = stdout_handle {
                let reader = BufReader::with_capacity(64 * 1024, out); // 64KB buffer
                for line in reader.lines() {
                    if let Ok(line) = line {
                        // CSV format: pts_time,dts_time,size,flags
                        let parts: Vec<&str> = line.split(',').collect();
                        if parts.len() >= 3 {
                            // Try pts_time first, fall back to dts_time
                            let timestamp = parts[0]
                                .parse::<f64>()
                                .ok()
                                .or_else(|| parts[1].parse::<f64>().ok());
                            let size = parts[2].parse::<u64>().ok();

                            if let (Some(ts), Some(sz)) = (timestamp, size) {
                                // flags field contains 'K' for keyframes
                                let frame_type = if parts.len() > 3 && parts[3].contains('K') {
                                    Some("I".to_string())
                                } else {
                                    None
                                };
                                results.push((ts, sz, frame_type));
                            }
                        }
                    }
                }
            }
            results
        }))
        .unwrap_or_else(|_| Vec::new())
    });

    let stderr_thread = thread::spawn(move || {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut stderr = Vec::new();
            if let Some(mut err) = stderr_handle {
                err.read_to_end(&mut stderr).ok();
            }
            stderr
        }))
        .unwrap_or_else(|_| Vec::new())
    });

    // Wait with timeout (3 minutes for packet mode - faster than frame mode)
    let timeout = Duration::from_secs(180);
    let start = std::time::Instant::now();

    let status = loop {
        if start.elapsed() > timeout {
            error!(
                "ffprobe (packet mode) timed out after {} seconds for stream {}",
                timeout.as_secs(),
                stream_index
            );
            let _ = child.kill();
            return Err(format!(
                "ffprobe timed out after {} seconds",
                timeout.as_secs()
            ));
        }

        match child.try_wait() {
            Ok(Some(status)) => {
                debug!(
                    "ffprobe (packet mode) completed for stream {} after {:.2}s",
                    stream_index,
                    start.elapsed().as_secs_f64()
                );
                break status;
            }
            Ok(None) => {
                thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                let _ = child.kill();
                return Err(format!("Failed to wait for ffprobe: {}", e));
            }
        }
    };

    let result = stdout_thread
        .join()
        .map_err(|_| "Failed to join stdout thread")?;
    let stderr = stderr_thread
        .join()
        .map_err(|_| "Failed to join stderr thread")?;

    if !status.success() {
        let err_msg = String::from_utf8_lossy(&stderr);
        error!(
            "ffprobe (packet mode) failed for stream {}: {}",
            stream_index, err_msg
        );
        return Err(format!("ffprobe failed: {}", err_msg));
    }

    if result.is_empty() {
        error!("No packets found for stream {}", stream_index);
        return Err(format!("No packets found for stream {}", stream_index));
    }

    info!(
        "Successfully parsed {} packets for stream {} in {:.2}s",
        result.len(),
        stream_index,
        start.elapsed().as_secs_f64()
    );
    Ok(result)
}

/// Parse ffprobe frame data for a specific stream (ACCURATE MODE)
///
/// This is slower but provides frame-level detail including picture type (I/P/B frames).
/// Use parse_ffprobe_packets for faster analysis when frame types aren't critical.
///
/// Returns a vector of (timestamp, size, frame_type) tuples
pub fn parse_ffprobe_frames(
    path: &str,
    stream_index: i32,
) -> Result<Vec<(f64, u64, Option<String>)>, String> {
    info!(
        "parse_ffprobe_frames: file={}, stream_index={}",
        path, stream_index
    );

    // Find ffprobe binary (needed for release builds where PATH isn't inherited)
    let ffprobe_cmd = find_command("ffprobe").unwrap_or_else(|| "ffprobe".to_string());
    debug!("Using ffprobe command: {}", ffprobe_cmd);

    // Spawn ffprobe process - use multiple timestamp fields for compatibility
    debug!("Spawning ffprobe for stream {}", stream_index);
    let mut child = Command::new(&ffprobe_cmd)
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
    // Use panic::catch_unwind to prevent zombies if thread panics
    let stdout_thread = thread::spawn(move || {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut stdout = Vec::new();
            if let Some(mut out) = stdout_handle {
                out.read_to_end(&mut stdout).ok();
            }
            stdout
        }))
        .unwrap_or_else(|_| Vec::new())
    });

    // Spawn thread to read stderr with panic handling
    let stderr_thread = thread::spawn(move || {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut stderr = Vec::new();
            if let Some(mut err) = stderr_handle {
                err.read_to_end(&mut stderr).ok();
            }
            stderr
        }))
        .unwrap_or_else(|_| Vec::new())
    });

    // Wait for completion with timeout (5 minutes max)
    let timeout = Duration::from_secs(300);
    let start = std::time::Instant::now();
    debug!(
        "Waiting for ffprobe to complete (timeout: {}s)",
        timeout.as_secs()
    );

    let status = loop {
        if start.elapsed() > timeout {
            error!(
                "ffprobe timed out after {} seconds for stream {} in file: {}",
                timeout.as_secs(),
                stream_index,
                path
            );
            let _ = child.kill();
            return Err(format!(
                "ffprobe timed out after {} seconds",
                timeout.as_secs()
            ));
        }

        match child.try_wait() {
            Ok(Some(status)) => {
                debug!(
                    "ffprobe completed for stream {} after {:.2}s",
                    stream_index,
                    start.elapsed().as_secs_f64()
                );
                break status;
            }
            Ok(None) => {
                // Still running, sleep briefly
                thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                error!("Failed to wait for ffprobe: {}", e);
                let _ = child.kill();
                return Err(format!("Failed to wait for ffprobe: {}", e));
            }
        }
    };

    // Collect output from threads
    debug!("Collecting ffprobe output from threads");
    let stdout = stdout_thread.join().map_err(|_| {
        error!("Failed to join stdout thread");
        "Failed to join stdout thread"
    })?;
    let stderr = stderr_thread.join().map_err(|_| {
        error!("Failed to join stderr thread");
        "Failed to join stderr thread"
    })?;

    if !status.success() {
        let err_msg = String::from_utf8_lossy(&stderr);
        error!(
            "ffprobe failed for stream {} in {}: {}",
            stream_index, path, err_msg
        );
        return Err(format!("ffprobe failed: {}", err_msg));
    }

    // Log any warnings from stderr
    if !stderr.is_empty() {
        let err_msg = String::from_utf8_lossy(&stderr);
        debug!("ffprobe stderr for stream {}: {}", stream_index, err_msg);
    }

    let json_str = String::from_utf8_lossy(&stdout);
    debug!(
        "ffprobe output size for stream {}: {} bytes",
        stream_index,
        json_str.len()
    );

    // Check if output is empty
    if json_str.trim().is_empty() {
        error!(
            "ffprobe returned empty output for stream {} in {}",
            stream_index, path
        );
        return Err(format!(
            "ffprobe returned empty output for stream {}",
            stream_index
        ));
    }

    debug!("Parsing JSON output for stream {}", stream_index);
    let parsed: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
        error!(
            "Failed to parse JSON for stream {} in {}: {}",
            stream_index, path, e
        );
        format!("Failed to parse JSON: {}", e)
    })?;

    let frames = parsed["frames"].as_array().ok_or_else(|| {
        error!(
            "No frames array found in ffprobe output for stream {} in {}",
            stream_index, path
        );
        format!(
            "No frames array found in ffprobe output for stream {}",
            stream_index
        )
    })?;

    debug!(
        "Found {} raw frames for stream {} in {}",
        frames.len(),
        stream_index,
        path
    );

    if frames.is_empty() {
        error!("No frames found for stream {} in {}", stream_index, path);
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
            "Skipped {} frames with missing data for stream {} ({}%)",
            skipped,
            stream_index,
            (skipped as f64 / frames.len() as f64) * 100.0
        );
    }

    if result.is_empty() {
        error!(
            "All frames had invalid data for stream {} in {} (skipped: {})",
            stream_index, path, skipped
        );
        return Err(format!(
            "All frames had invalid data for stream {}",
            stream_index
        ));
    }

    info!(
        "Successfully parsed {} frames for stream {} in {} (skipped: {})",
        result.len(),
        stream_index,
        path,
        skipped
    );
    Ok(result)
}

/// Aggregate frames into time intervals for bitrate calculation
pub fn aggregate_bitrate_intervals(
    frames: Vec<(f64, u64, Option<String>)>,
    interval_seconds: f64,
    duration: f64,
) -> Vec<BitrateDataPoint> {
    debug!(
        "aggregate_bitrate_intervals: {} frames, interval={:.2}s, duration={:.2}s",
        frames.len(),
        interval_seconds,
        duration
    );

    if frames.is_empty() {
        debug!("No frames to aggregate");
        return Vec::new();
    }

    let num_intervals = (duration / interval_seconds).ceil() as usize;
    debug!("Creating {} intervals", num_intervals);
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

    let data_points: Vec<BitrateDataPoint> = intervals
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
        .collect();

    debug!(
        "Aggregated into {} data points (avg bitrate: {})",
        data_points.len(),
        if !data_points.is_empty() {
            data_points.iter().map(|d| d.bitrate).sum::<u64>() / data_points.len() as u64
        } else {
            0
        }
    );

    data_points
}

/// Calculate statistics from bitrate data points
pub fn calculate_statistics(data_points: &[BitrateDataPoint]) -> BitrateStatistics {
    debug!("calculate_statistics: {} data points", data_points.len());

    if data_points.is_empty() {
        debug!("No data points to calculate statistics");
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

    let stats = BitrateStatistics {
        min_bitrate,
        max_bitrate,
        avg_bitrate,
        median_bitrate,
        std_deviation,
        peak_intervals: peak_intervals.clone(),
        total_frames: data_points.len(),
    };

    debug!(
        "Statistics calculated: min={}, max={}, avg={}, median={}, std_dev={:.2}, peaks={}",
        min_bitrate,
        max_bitrate,
        avg_bitrate,
        median_bitrate,
        std_deviation,
        peak_intervals.len()
    );

    stats
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

/// Parse ffprobe data using the best available method
///
/// Tries packet mode first (fast), falls back to frame mode if needed.
/// This provides the best balance of speed and reliability.
pub fn parse_ffprobe_auto(
    path: &str,
    stream_index: i32,
    prefer_accuracy: bool,
) -> Result<Vec<(f64, u64, Option<String>)>, String> {
    if prefer_accuracy {
        // User explicitly requested accurate mode
        info!(
            "Using accurate frame mode for stream {} (user preference)",
            stream_index
        );
        return parse_ffprobe_frames(path, stream_index);
    }

    // Try fast packet mode first
    match parse_ffprobe_packets(path, stream_index) {
        Ok(packets) if !packets.is_empty() => {
            info!(
                "Fast packet mode succeeded for stream {} ({} packets)",
                stream_index,
                packets.len()
            );
            Ok(packets)
        }
        Ok(_) => {
            warn!(
                "Packet mode returned no data for stream {}, falling back to frame mode",
                stream_index
            );
            parse_ffprobe_frames(path, stream_index)
        }
        Err(e) => {
            warn!(
                "Packet mode failed for stream {} ({}), falling back to frame mode",
                stream_index, e
            );
            parse_ffprobe_frames(path, stream_index)
        }
    }
}

/// Parse ffprobe data with automatic sampling for large files
///
/// For files larger than SAMPLING_THRESHOLD_BYTES, uses sampling mode which
/// reads only portions of the file and extrapolates the results.
/// This can provide 10-100x speedup for very large files.
///
/// Returns: (data, was_sampled)
pub fn parse_ffprobe_sampled(
    path: &str,
    stream_index: i32,
    duration: f64,
    file_size: u64,
) -> Result<(Vec<(f64, u64, Option<String>)>, bool), String> {
    // Check if file is large enough to warrant sampling
    if file_size < SAMPLING_THRESHOLD_BYTES {
        debug!(
            "File size {} bytes < threshold {} bytes, using full analysis",
            file_size, SAMPLING_THRESHOLD_BYTES
        );
        let data = parse_ffprobe_packets(path, stream_index)?;
        return Ok((data, false));
    }

    info!(
        "Large file detected ({:.2} GB), using sampling mode for stream {}",
        file_size as f64 / 1024.0 / 1024.0 / 1024.0,
        stream_index
    );

    // Calculate sample positions distributed across the file
    // We want samples at: start, evenly distributed middle sections, and end
    let mut sample_positions: Vec<f64> = Vec::with_capacity(SAMPLE_COUNT);

    if duration <= SAMPLE_DURATION_SECS * SAMPLE_COUNT as f64 {
        // File is short enough to analyze fully despite large size (high bitrate)
        debug!("Duration {:.1}s is short, analyzing fully", duration);
        let data = parse_ffprobe_packets(path, stream_index)?;
        return Ok((data, false));
    }

    // Distribute samples evenly across the duration
    let interval = duration / SAMPLE_COUNT as f64;
    for i in 0..SAMPLE_COUNT {
        let pos = i as f64 * interval;
        sample_positions.push(pos);
    }

    debug!(
        "Sampling {} positions across {:.1}s duration: {:?}",
        SAMPLE_COUNT, duration, sample_positions
    );

    // Collect samples from each position
    let mut all_packets: Vec<(f64, u64, Option<String>)> = Vec::new();
    let mut total_sample_duration = 0.0;

    for (idx, start_pos) in sample_positions.iter().enumerate() {
        // Format: "start%+duration" - read SAMPLE_DURATION_SECS starting at start_pos
        let read_interval = format!("{}%+{}", start_pos, SAMPLE_DURATION_SECS);

        debug!(
            "Reading sample {}/{} at position {:.1}s",
            idx + 1,
            SAMPLE_COUNT,
            start_pos
        );

        match parse_ffprobe_packets_internal(path, stream_index, Some(&read_interval)) {
            Ok(packets) => {
                debug!("Sample {} returned {} packets", idx + 1, packets.len());
                all_packets.extend(packets);
                total_sample_duration += SAMPLE_DURATION_SECS;
            }
            Err(e) => {
                warn!("Sample {} failed: {}", idx + 1, e);
                // Continue with other samples
            }
        }
    }

    if all_packets.is_empty() {
        warn!(
            "All samples failed for stream {}, falling back to full analysis",
            stream_index
        );
        let data = parse_ffprobe_packets(path, stream_index)?;
        return Ok((data, false));
    }

    info!(
        "Sampling complete: {} packets from {:.1}s of {:.1}s total ({:.1}% coverage)",
        all_packets.len(),
        total_sample_duration,
        duration,
        (total_sample_duration / duration) * 100.0
    );

    Ok((all_packets, true))
}

/// Extrapolate sampled data to create full duration estimate
///
/// Takes sampled packet data and creates interpolated data points
/// for the full duration. This is used for visualization when sampling.
pub fn extrapolate_sampled_data(
    sampled_data: &[(f64, u64, Option<String>)],
    sampled_duration: f64,
    full_duration: f64,
    interval_seconds: f64,
) -> Vec<BitrateDataPoint> {
    if sampled_data.is_empty() {
        return Vec::new();
    }

    // Calculate average bitrate from samples
    let total_bytes: u64 = sampled_data.iter().map(|(_, size, _)| size).sum();
    let avg_bitrate = if sampled_duration > 0.0 {
        ((total_bytes * 8) as f64 / sampled_duration) as u64
    } else {
        0
    };

    debug!(
        "Extrapolating: {} bytes over {:.1}s = {} bps average",
        total_bytes, sampled_duration, avg_bitrate
    );

    // First, aggregate the actual sampled data into intervals
    let num_intervals = (full_duration / interval_seconds).ceil() as usize;
    let mut intervals: Vec<(u64, usize)> = vec![(0, 0); num_intervals]; // (total_bytes, count)

    for (timestamp, size, _) in sampled_data {
        let interval_idx = (*timestamp / interval_seconds).floor() as usize;
        if interval_idx < num_intervals {
            intervals[interval_idx].0 += size;
            intervals[interval_idx].1 += 1;
        }
    }

    // Create data points, using actual data where available and avg for gaps
    let data_points: Vec<BitrateDataPoint> = intervals
        .into_iter()
        .enumerate()
        .map(|(idx, (total_size, count))| {
            let bitrate = if count > 0 {
                // We have actual data for this interval
                ((total_size * 8) as f64 / interval_seconds) as u64
            } else {
                // No data - use average (this interval was not sampled)
                avg_bitrate
            };
            BitrateDataPoint {
                timestamp: idx as f64 * interval_seconds,
                bitrate,
                frame_type: None,
            }
        })
        .collect();

    info!(
        "Extrapolated {} data points (avg bitrate: {} bps)",
        data_points.len(),
        avg_bitrate
    );

    data_points
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
