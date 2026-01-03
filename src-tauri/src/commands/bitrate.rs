//! Bitrate analysis Tauri commands
//!
//! This module contains all Tauri commands for bitrate analysis operations,
//! including progress reporting via Tauri window events.

use log::{debug, error, info, warn};
use std::sync::atomic::Ordering;
use tauri::Emitter;

use crate::bitrate::{
    self, aggregate_bitrate_intervals, calculate_statistics, clear_cache, get_cached_analysis,
    parse_ffprobe_frames, save_to_cache, sort_streams_audio_first, JobStartResult,
};
use crate::files::get_file_metadata;
use crate::media::get_media_streams;
use crate::types::{
    BitrateAnalysis, BitrateDataPoint, BitrateProgress, JobStatus, OverallBitrateAnalysis,
    StreamContribution, StreamType,
};

/// Analyze bitrate for a specific stream in a media file
#[tauri::command]
pub async fn analyze_stream_bitrate(
    path: String,
    stream_index: i32,
    interval_seconds: f64,
    window: tauri::Window,
) -> Result<BitrateAnalysis, String> {
    let path_clone = path.clone();
    let window_clone = window.clone();

    // Try to start a new job - if one is already running, return error
    let job = match bitrate::start_job(&path_clone) {
        JobStartResult::Started(job) => job,
        JobStartResult::AlreadyRunning(job_id) => {
            return Err(format!(
                "Analysis already in progress for this file (job {})",
                job_id
            ));
        }
    };

    let job_id = job.id;
    let cancelled = job.cancelled.clone();
    let path_for_cleanup = path_clone.clone();

    // Run in blocking thread to avoid blocking async runtime
    let result = tauri::async_runtime::spawn_blocking(move || {
        info!(
            "Starting stream bitrate analysis: stream={}, path={}, job_id={}",
            stream_index, path_clone, job_id
        );

        // Stage 1: Get stream info
        window_clone
            .emit(
                "bitrate-progress",
                BitrateProgress {
                    current: 0,
                    total: 100,
                    percentage: 0.0,
                    stage: "Getting stream info...".to_string(),
                },
            )
            .ok();

        // Check for cancellation
        if cancelled.load(Ordering::SeqCst) {
            return Err("Analysis cancelled".to_string());
        }

        debug!("Stage 1: Getting stream info");
        let streams = get_media_streams(path_clone.clone())?;
        let stream = streams
            .streams
            .iter()
            .find(|s| s.index == stream_index)
            .ok_or("Stream not found")?;

        // Stage 2: Get duration
        window_clone
            .emit(
                "bitrate-progress",
                BitrateProgress {
                    current: 10,
                    total: 100,
                    percentage: 10.0,
                    stage: "Reading file metadata...".to_string(),
                },
            )
            .ok();

        let metadata_json = get_file_metadata(path_clone.clone())?;
        let metadata: serde_json::Value = serde_json::from_str(
            &metadata_json
                .ffprobe_data
                .ok_or("No ffprobe data available")?,
        )
        .map_err(|e| format!("Failed to parse metadata: {}", e))?;

        let duration = metadata["format"]["duration"]
            .as_str()
            .and_then(|s| s.parse::<f64>().ok())
            .ok_or("Could not determine duration")?;

        // Check for cancellation before heavy operation
        if cancelled.load(Ordering::SeqCst) {
            return Err("Analysis cancelled".to_string());
        }

        // Stage 3: Parse frames (this is the heavy operation)
        debug!(
            "Stage 3: Starting frame parsing for stream {}",
            stream_index
        );
        window_clone
            .emit(
                "bitrate-progress",
                BitrateProgress {
                    current: 20,
                    total: 100,
                    percentage: 20.0,
                    stage: "Analyzing frames...".to_string(),
                },
            )
            .ok();

        let frames = parse_ffprobe_frames(&path_clone, stream_index)?;
        info!(
            "Frame parsing complete for stream {}: {} frames",
            stream_index,
            frames.len()
        );

        // Stage 4: Aggregate data
        window_clone
            .emit(
                "bitrate-progress",
                BitrateProgress {
                    current: 80,
                    total: 100,
                    percentage: 80.0,
                    stage: "Aggregating bitrate data...".to_string(),
                },
            )
            .ok();

        let data_points = aggregate_bitrate_intervals(frames, interval_seconds, duration);

        // Stage 5: Calculate statistics
        window_clone
            .emit(
                "bitrate-progress",
                BitrateProgress {
                    current: 90,
                    total: 100,
                    percentage: 90.0,
                    stage: "Calculating statistics...".to_string(),
                },
            )
            .ok();

        let statistics = calculate_statistics(&data_points);

        // Stage 6: Complete
        window_clone
            .emit(
                "bitrate-progress",
                BitrateProgress {
                    current: 100,
                    total: 100,
                    percentage: 100.0,
                    stage: "Complete".to_string(),
                },
            )
            .ok();

        Ok(BitrateAnalysis {
            path: path_clone,
            stream_index,
            stream_type: stream.stream_type.clone(),
            duration,
            data_points,
            statistics,
        })
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?;

    // Clean up the job
    bitrate::complete_job(&path_for_cleanup, job_id);

    result
}

/// Analyze overall bitrate for a media file (all streams combined)
#[tauri::command]
pub async fn analyze_overall_bitrate(
    path: String,
    interval_seconds: f64,
    window: tauri::Window,
) -> Result<OverallBitrateAnalysis, String> {
    let path_clone = path.clone();
    let window_clone = window.clone();

    // Check cache first
    if let Some(cached) = get_cached_analysis(&path_clone) {
        info!("Returning cached analysis for: {}", path_clone);
        // Emit complete progress
        window_clone
            .emit(
                "bitrate-progress",
                BitrateProgress {
                    current: 100,
                    total: 100,
                    percentage: 100.0,
                    stage: "Loaded from cache".to_string(),
                },
            )
            .ok();
        // Return cached result with updated path (in case file was moved/copied)
        return Ok(OverallBitrateAnalysis {
            path: path_clone,
            from_cache: true,
            ..cached
        });
    }

    // Try to start a new job - if one is already running, return error
    let job = match bitrate::start_job(&path_clone) {
        JobStartResult::Started(job) => job,
        JobStartResult::AlreadyRunning(job_id) => {
            return Err(format!(
                "Analysis already in progress for this file (job {})",
                job_id
            ));
        }
    };

    let job_id = job.id;
    let cancelled = job.cancelled.clone();
    let path_for_cleanup = path_clone.clone();

    // Run in blocking thread to avoid blocking async runtime
    let result = tauri::async_runtime::spawn_blocking(move || {
        info!(
            "Starting overall bitrate analysis: path={}, job_id={}",
            path_clone, job_id
        );

        // Stage 1: Get all streams
        window_clone
            .emit(
                "bitrate-progress",
                BitrateProgress {
                    current: 0,
                    total: 100,
                    percentage: 0.0,
                    stage: "Getting stream information...".to_string(),
                },
            )
            .ok();
        // Small yield to allow event delivery
        std::thread::sleep(std::time::Duration::from_millis(10));

        debug!("Stage 1: Getting streams");
        let streams = get_media_streams(path_clone.clone())?;
        info!("Found {} total streams", streams.streams.len());

        // Stage 2: Get duration
        window_clone
            .emit(
                "bitrate-progress",
                BitrateProgress {
                    current: 5,
                    total: 100,
                    percentage: 5.0,
                    stage: "Reading file metadata...".to_string(),
                },
            )
            .ok();
        // Small yield to allow event delivery
        std::thread::sleep(std::time::Duration::from_millis(10));

        let metadata_json = get_file_metadata(path_clone.clone())?;
        let metadata: serde_json::Value = serde_json::from_str(
            &metadata_json
                .ffprobe_data
                .ok_or("No ffprobe data available")?,
        )
        .map_err(|e| format!("Failed to parse metadata: {}", e))?;

        let duration = metadata["format"]["duration"]
            .as_str()
            .and_then(|s| s.parse::<f64>().ok())
            .ok_or("Could not determine duration")?;

        // Check for cancellation
        if cancelled.load(Ordering::SeqCst) {
            return Err("Analysis cancelled".to_string());
        }

        // Count video/audio streams for progress tracking
        // Sort to process audio first (faster feedback)
        let mut analysis_streams: Vec<_> = streams
            .streams
            .iter()
            .filter(|s| s.stream_type == StreamType::Video || s.stream_type == StreamType::Audio)
            .collect();

        // Sort: audio first, then video (audio is faster to process)
        sort_streams_audio_first(&mut analysis_streams);

        let total_streams = analysis_streams.len();
        info!("Will analyze {} video/audio streams", total_streams);
        let num_intervals = (duration / interval_seconds).ceil() as usize;
        let mut combined_intervals: Vec<u64> = vec![0; num_intervals];
        let mut stream_data: Vec<(i32, StreamType, Option<String>, Vec<u64>)> = Vec::new();
        let mut _total_bitrate = 0u64;

        // Stage 3: Analyze each stream (10-90% progress)
        debug!("Stage 3: Starting stream-by-stream analysis");
        let mut analyzed_count = 0;
        for (idx, stream) in analysis_streams.iter().enumerate() {
            info!(
                "Analyzing stream {}/{}: index={}, type={:?}, codec={:?}",
                idx + 1,
                total_streams,
                stream.index,
                stream.stream_type,
                stream.codec_name
            );

            let progress = 10.0 + (idx as f64 / total_streams as f64) * 80.0;
            // Check for cancellation before each stream
            if cancelled.load(Ordering::SeqCst) {
                info!("Job {} cancelled during stream analysis", job_id);
                return Err("Analysis cancelled".to_string());
            }

            let stream_type_name = match stream.stream_type {
                StreamType::Audio => "audio",
                StreamType::Video => "video",
                _ => "stream",
            };

            window_clone
                .emit(
                    "bitrate-progress",
                    BitrateProgress {
                        current: progress as usize,
                        total: 100,
                        percentage: progress,
                        stage: format!(
                            "Analyzing {} {}/{} ({})...",
                            stream_type_name,
                            idx + 1,
                            total_streams,
                            stream.codec_name.as_deref().unwrap_or("unknown")
                        ),
                    },
                )
                .ok();
            // Small yield to allow event delivery
            std::thread::sleep(std::time::Duration::from_millis(10));

            // Get frames for this stream with timeout and error handling
            debug!(
                "Calling parse_ffprobe_frames for stream {} ({})",
                stream.index, stream_type_name
            );
            let frames = match parse_ffprobe_frames(&path_clone, stream.index) {
                Ok(f) => {
                    analyzed_count += 1;
                    f
                }
                Err(e) => {
                    warn!(
                        "Failed to parse stream {} ({}): {}",
                        stream.index,
                        stream.codec_name.as_deref().unwrap_or("unknown"),
                        e
                    );
                    continue;
                }
            };

            // Track per-stream intervals
            let mut stream_intervals: Vec<u64> = vec![0; num_intervals];

            // Aggregate this stream's contribution
            for (timestamp, size, _) in frames {
                let interval_idx = (timestamp / interval_seconds).floor() as usize;
                if interval_idx < num_intervals {
                    combined_intervals[interval_idx] += size;
                    stream_intervals[interval_idx] += size;
                }
            }

            // Store stream data for later
            stream_data.push((
                stream.index,
                stream.stream_type.clone(),
                stream.codec_name.clone(),
                stream_intervals,
            ));

            // Calculate stream contribution percentage
            if let Some(bit_rate_str) = &stream.bit_rate {
                if let Ok(bit_rate) = bit_rate_str.parse::<u64>() {
                    _total_bitrate += bit_rate;
                }
            }
        }

        // Check if we analyzed any streams
        if analyzed_count == 0 {
            error!("Failed to analyze any streams in the file");
            return Err("Failed to analyze any streams in the file".to_string());
        }

        info!(
            "Successfully analyzed {}/{} streams",
            analyzed_count, total_streams
        );

        // Stage 4: Aggregate data
        window_clone
            .emit(
                "bitrate-progress",
                BitrateProgress {
                    current: 90,
                    total: 100,
                    percentage: 90.0,
                    stage: "Aggregating bitrate data...".to_string(),
                },
            )
            .ok();
        // Small yield to allow event delivery
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Calculate combined total before consuming combined_intervals
        let combined_total: u64 = combined_intervals.iter().sum();

        let data_points: Vec<BitrateDataPoint> = combined_intervals
            .into_iter()
            .enumerate()
            .map(|(idx, total_size)| {
                let bitrate = ((total_size * 8) as f64 / interval_seconds) as u64;
                BitrateDataPoint {
                    timestamp: idx as f64 * interval_seconds,
                    bitrate,
                    frame_type: None,
                }
            })
            .collect();

        // Calculate stream contributions with per-stream data points
        let mut stream_contributions: Vec<StreamContribution> = Vec::new();
        for (stream_index, stream_type, codec_name, intervals) in stream_data {
            // Calculate this stream's total bitrate from actual data
            let stream_total: u64 = intervals.iter().sum();
            let percentage = if combined_total > 0 {
                (stream_total as f64 / combined_total as f64) * 100.0
            } else {
                0.0
            };

            // Convert intervals to data points
            let stream_data_points: Vec<BitrateDataPoint> = intervals
                .into_iter()
                .enumerate()
                .map(|(idx, total_size)| {
                    let bitrate = ((total_size * 8) as f64 / interval_seconds) as u64;
                    BitrateDataPoint {
                        timestamp: idx as f64 * interval_seconds,
                        bitrate,
                        frame_type: None,
                    }
                })
                .collect();

            stream_contributions.push(StreamContribution {
                stream_index,
                stream_type,
                codec_name: codec_name.unwrap_or_default(),
                percentage,
                data_points: stream_data_points,
            });
        }

        // Stage 5: Calculate statistics
        window_clone
            .emit(
                "bitrate-progress",
                BitrateProgress {
                    current: 95,
                    total: 100,
                    percentage: 95.0,
                    stage: "Calculating statistics...".to_string(),
                },
            )
            .ok();
        // Small yield to allow event delivery
        std::thread::sleep(std::time::Duration::from_millis(10));

        let statistics = calculate_statistics(&data_points);

        // Complete
        window_clone
            .emit(
                "bitrate-progress",
                BitrateProgress {
                    current: 100,
                    total: 100,
                    percentage: 100.0,
                    stage: "Complete".to_string(),
                },
            )
            .ok();

        Ok(OverallBitrateAnalysis {
            path: path_clone,
            duration,
            data_points,
            statistics,
            stream_contributions,
            from_cache: false,
        })
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?;

    // Clean up the job
    bitrate::complete_job(&path_for_cleanup, job_id);

    // Cache the successful result
    if let Ok(ref analysis) = result {
        if let Err(e) = save_to_cache(&path_for_cleanup, analysis) {
            warn!("Failed to cache analysis result: {}", e);
        }
    }

    result
}

/// Cancel an ongoing bitrate analysis for a file
#[tauri::command]
pub async fn cancel_bitrate_analysis(path: String) -> Result<bool, String> {
    Ok(bitrate::cancel_job(&path))
}

/// Get the status of all active bitrate analysis jobs
#[tauri::command]
pub async fn get_bitrate_job_status() -> Result<Vec<JobStatus>, String> {
    Ok(bitrate::get_active_jobs())
}

/// Clear the bitrate analysis cache
#[tauri::command]
pub async fn clear_bitrate_cache_cmd() -> Result<usize, String> {
    clear_cache()
}
