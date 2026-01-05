//! Bitrate analysis Tauri commands
//!
//! This module contains all Tauri commands for bitrate analysis operations,
//! including progress reporting via Tauri window events.
//!
//! Note: Caching is handled by the frontend via SQLite database.
//! The backend focuses purely on analysis - the frontend checks the DB cache
//! before calling these analysis functions and saves results after completion.
//!
//! Performance optimizations:
//! - Uses packet-mode ffprobe (fast) by default, falls back to frame-mode if needed
//! - Parallel stream processing using rayon for multi-stream files
//! - Streaming CSV parsing for reduced memory overhead

use log::{debug, error, info, warn};
use rayon::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tauri::Emitter;

use crate::bitrate::{
    aggregate_bitrate_intervals, calculate_statistics, compute_file_hash, parse_ffprobe_auto,
    parse_ffprobe_sampled, SAMPLE_COUNT, SAMPLE_DURATION_SECS, SAMPLING_THRESHOLD_BYTES,
};
use crate::files::get_file_metadata;
use crate::jobs::{self, JobProgress, JobStartResult, JobType};
use crate::media::get_media_streams;
use crate::types::{
    BitrateAnalysis, BitrateDataPoint, BitrateProgress, JobStatus, OverallBitrateAnalysis,
    QueueStatus, StreamContribution, StreamType,
};

/// Compute a file hash for cache validation
///
/// This hash is based on file size, modification time, and sample bytes from
/// the beginning and end of the file. It's fast to compute and good for
/// detecting file changes without reading the entire file.
#[tauri::command]
pub fn compute_file_hash_cmd(path: String) -> Result<String, String> {
    debug!("compute_file_hash_cmd called: path={}", path);
    let result = compute_file_hash(&path);
    match &result {
        Ok(hash) => debug!("Computed file hash for {}: {}", path, hash),
        Err(e) => error!("Failed to compute file hash for {}: {}", path, e),
    }
    result
}

/// Analyze bitrate for a specific stream in a media file
#[tauri::command]
pub async fn analyze_stream_bitrate(
    path: String,
    stream_index: i32,
    interval_seconds: f64,
    window: tauri::Window,
) -> Result<BitrateAnalysis, String> {
    info!(
        "analyze_stream_bitrate command: path={}, stream_index={}, interval={}s",
        path, stream_index, interval_seconds
    );

    let path_clone = path.clone();
    let window_clone = window.clone();

    // Compute file hash for job ID
    debug!("Computing file hash for job ID");
    let file_hash = compute_file_hash(&path_clone)?;
    debug!("File hash: {}", file_hash);
    // let file_hash_clone = file_hash.clone();

    // Enqueue job - starts immediately if slot available, otherwise queues
    let (job_id, cancelled) =
        match jobs::enqueue_job(&path_clone, &file_hash, JobType::BitrateAnalysis) {
            JobStartResult::Started(id) | JobStartResult::Queued(id) => {
                // Get cancellation flag from the job
                let cancel_flag = jobs::get_job_cancel_flag(&path_clone)
                    .ok_or("Failed to get job cancellation flag")?;
                (id, cancel_flag)
            }
            JobStartResult::AlreadyExists(job_id) => {
                return Err(format!(
                    "Analysis already queued or in progress for this file (job {})",
                    job_id
                ));
            }
        };

    let path_for_cleanup = path_clone.clone();

    // Emit queue update
    window
        .emit("job-queue-update", jobs::get_queue_status())
        .ok();

    // Run in blocking thread to avoid blocking async runtime
    debug!("Spawning blocking thread for analysis");
    let result = tauri::async_runtime::spawn_blocking(move || {
        info!(
            "Starting stream bitrate analysis: stream={}, path={}, job_id={}, interval={}s",
            stream_index, path_clone, job_id, interval_seconds
        );

        // Track start time for ETA calculation
        let analysis_start = std::time::Instant::now();

        // Helper to emit progress and update job state with ETA
        let emit_progress = |current: usize, total: usize, percentage: f64, stage: String| {
            let elapsed = analysis_start.elapsed().as_secs_f64();

            // Calculate ETA based on elapsed time and percentage
            let eta_seconds = if percentage > 5.0 && percentage < 100.0 {
                let remaining_percentage = 100.0 - percentage;
                Some((elapsed / percentage) * remaining_percentage)
            } else {
                None
            };

            let progress = BitrateProgress {
                current,
                total,
                percentage,
                stage: stage.clone(),
                eta_seconds,
                elapsed_seconds: Some(elapsed),
                using_sampling: None,
                stream_count: Some(1),
                current_stream: Some(1),
            };
            window_clone.emit("bitrate-progress", &progress).ok();
            jobs::update_job_progress(
                &path_clone,
                JobProgress {
                    current,
                    total,
                    percentage,
                    stage,
                },
            );
        };

        // Stage 1: Get stream info
        emit_progress(0, 100, 0.0, "Getting stream info...".to_string());

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
        emit_progress(10, 100, 10.0, "Reading file metadata...".to_string());

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
        emit_progress(20, 100, 20.0, "Analyzing frames...".to_string());

        // Use auto mode (tries fast packet mode first, falls back to frame mode)
        let frames = parse_ffprobe_auto(&path_clone, stream_index, false)?;
        info!(
            "Frame parsing complete for stream {}: {} frames",
            stream_index,
            frames.len()
        );

        // Stage 4: Aggregate data
        emit_progress(80, 100, 80.0, "Aggregating bitrate data...".to_string());

        let data_points = aggregate_bitrate_intervals(frames, interval_seconds, duration);

        // Stage 5: Calculate statistics
        emit_progress(90, 100, 90.0, "Calculating statistics...".to_string());

        let statistics = calculate_statistics(&data_points);

        // Stage 6: Complete
        let total_elapsed = analysis_start.elapsed().as_secs_f64();
        emit_progress(
            100,
            100,
            100.0,
            format!("Complete in {:.1}s", total_elapsed),
        );
        info!(
            "Stream {} bitrate analysis complete in {:.2}s",
            stream_index, total_elapsed
        );

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
    .map_err(|e| format!("Task join error: {}", e));

    // Always clean up the job, regardless of success/failure
    debug!("Cleaning up job for {}", path_for_cleanup);
    jobs::complete_job(&path_for_cleanup);

    // Emit queue update after completion
    window
        .emit("job-queue-update", jobs::get_queue_status())
        .ok();

    // Now propagate error if there was one
    match &result {
        Ok(_) => info!("Stream bitrate analysis completed successfully: {}", path),
        Err(e) => error!("Stream bitrate analysis failed for {}: {}", path, e),
    }

    result?
}

/// Analyze overall bitrate for a media file (all streams combined)
///
/// Note: Caching is handled by the frontend. The frontend should:
/// 1. Check the database cache before calling this function
/// 2. Save the result to the database after this function returns
#[tauri::command]
pub async fn analyze_overall_bitrate(
    path: String,
    interval_seconds: f64,
    window: tauri::Window,
) -> Result<OverallBitrateAnalysis, String> {
    info!(
        "analyze_overall_bitrate command: path={}, interval={}s",
        path, interval_seconds
    );

    let path_clone = path.clone();
    let window_clone = window.clone();

    // Compute file hash for job ID
    debug!("Computing file hash for job ID");
    let file_hash = compute_file_hash(&path_clone)?;
    debug!("File hash: {}", file_hash);
    // let file_hash_clone = file_hash.clone();

    // Enqueue job - starts immediately if slot available, otherwise queues
    debug!("Calling enqueue_job...");
    let enqueue_result = jobs::enqueue_job(&path_clone, &file_hash, JobType::BitrateAnalysis);
    debug!("enqueue_job returned: {:?}", enqueue_result);

    let (job_id, cancelled) = match enqueue_result {
        JobStartResult::Started(id) => {
            info!("Job started immediately with id: {}", id);
            // Get cancellation flag from the job
            let cancel_flag = jobs::get_job_cancel_flag(&path_clone)
                .ok_or("Failed to get job cancellation flag")?;
            (id, cancel_flag)
        }
        JobStartResult::Queued(id) => {
            info!("Job queued with id: {}", id);
            // Get cancellation flag from the job
            let cancel_flag = jobs::get_job_cancel_flag(&path_clone)
                .ok_or("Failed to get job cancellation flag")?;
            (id, cancel_flag)
        }
        JobStartResult::AlreadyExists(job_id) => {
            warn!("Job already exists: {}", job_id);
            return Err(format!(
                "Analysis already queued or in progress for this file (job {})",
                job_id
            ));
        }
    };
    debug!("Got job_id={}, proceeding to emit queue update", job_id);

    let path_for_cleanup = path_clone.clone();

    // Emit queue update
    window
        .emit("job-queue-update", jobs::get_queue_status())
        .ok();

    // Run in blocking thread to avoid blocking async runtime
    debug!("Spawning blocking thread for overall analysis");
    let result = tauri::async_runtime::spawn_blocking(move || {
        info!(
            "Starting overall bitrate analysis: path={}, job_id={}, interval={}s",
            path_clone, job_id, interval_seconds
        );

        // Track start time for ETA calculation
        let analysis_start = std::time::Instant::now();

        // Helper to emit progress and update job state with enhanced info
        let emit_progress_enhanced =
            |current: usize,
             total: usize,
             percentage: f64,
             stage: String,
             using_sampling: Option<bool>,
             stream_count: Option<usize>,
             current_stream: Option<usize>| {
                let elapsed = analysis_start.elapsed().as_secs_f64();

                // Calculate ETA based on elapsed time and percentage
                let eta_seconds = if percentage > 5.0 && percentage < 100.0 {
                    let remaining_percentage = 100.0 - percentage;
                    Some((elapsed / percentage) * remaining_percentage)
                } else {
                    None
                };

                let progress = BitrateProgress {
                    current,
                    total,
                    percentage,
                    stage: stage.clone(),
                    eta_seconds,
                    elapsed_seconds: Some(elapsed),
                    using_sampling,
                    stream_count,
                    current_stream,
                };
                window_clone.emit("bitrate-progress", &progress).ok();
                jobs::update_job_progress(
                    &path_clone,
                    JobProgress {
                        current,
                        total,
                        percentage,
                        stage,
                    },
                );
                // Small yield to allow event delivery
                std::thread::sleep(std::time::Duration::from_millis(10));
            };

        // Simple progress helper for initial stages
        let emit_progress = |current: usize, total: usize, percentage: f64, stage: String| {
            emit_progress_enhanced(current, total, percentage, stage, None, None, None);
        };

        // Stage 1: Get all streams
        emit_progress(0, 100, 0.0, "Getting stream information...".to_string());

        debug!("Stage 1: Getting streams");
        let streams = get_media_streams(path_clone.clone())?;
        let file_size = streams.total_size;
        info!(
            "Found {} total streams, file size: {:.2} GB",
            streams.streams.len(),
            file_size as f64 / 1024.0 / 1024.0 / 1024.0
        );

        // Check if we should use sampling mode for large files
        let use_sampling = file_size >= SAMPLING_THRESHOLD_BYTES;
        if use_sampling {
            info!(
                "Large file detected ({:.2} GB >= {:.2} GB threshold), will use sampling mode",
                file_size as f64 / 1024.0 / 1024.0 / 1024.0,
                SAMPLING_THRESHOLD_BYTES as f64 / 1024.0 / 1024.0 / 1024.0
            );
        }

        // Stage 2: Get duration
        emit_progress(5, 100, 5.0, "Reading file metadata...".to_string());

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
        let analysis_streams: Vec<_> = streams
            .streams
            .iter()
            .filter(|s| s.stream_type == StreamType::Video || s.stream_type == StreamType::Audio)
            .cloned()
            .collect();

        let total_streams = analysis_streams.len();
        info!(
            "Will analyze {} video/audio streams in parallel (sampling: {})",
            total_streams, use_sampling
        );
        let num_intervals = (duration / interval_seconds).ceil() as usize;

        // Stage 3: Analyze streams in PARALLEL using rayon
        debug!("Stage 3: Starting parallel stream analysis");
        let stage_msg = if use_sampling {
            format!(
                "Sampling {} streams ({} x {}s intervals)...",
                total_streams, SAMPLE_COUNT, SAMPLE_DURATION_SECS
            )
        } else {
            format!("Analyzing {} streams in parallel...", total_streams)
        };
        emit_progress_enhanced(
            10,
            100,
            10.0,
            stage_msg,
            Some(use_sampling),
            Some(total_streams),
            None,
        );

        // Shared progress counter for parallel execution
        let analyzed_count = Arc::new(AtomicUsize::new(0));
        let cancelled_clone = cancelled.clone();

        // Process streams in parallel
        let stream_results: Vec<_> = analysis_streams
            .par_iter()
            .filter_map(|stream| {
                // Check for cancellation
                if cancelled_clone.load(Ordering::SeqCst) {
                    return None;
                }

                let stream_type_name = match stream.stream_type {
                    StreamType::Audio => "audio",
                    StreamType::Video => "video",
                    _ => "stream",
                };

                debug!(
                    "Parallel: Analyzing {} stream {} ({})",
                    stream_type_name,
                    stream.index,
                    stream.codec_name.as_deref().unwrap_or("unknown")
                );

                // Use sampling for large files, otherwise fast packet mode
                let (frames, was_sampled) = if use_sampling {
                    match parse_ffprobe_sampled(&path_clone, stream.index, duration, file_size) {
                        Ok((f, sampled)) => {
                            analyzed_count.fetch_add(1, Ordering::SeqCst);
                            (f, sampled)
                        }
                        Err(e) => {
                            warn!(
                                "Failed to parse stream {} ({}): {}",
                                stream.index,
                                stream.codec_name.as_deref().unwrap_or("unknown"),
                                e
                            );
                            return None;
                        }
                    }
                } else {
                    match parse_ffprobe_auto(&path_clone, stream.index, false) {
                        Ok(f) => {
                            analyzed_count.fetch_add(1, Ordering::SeqCst);
                            (f, false)
                        }
                        Err(e) => {
                            warn!(
                                "Failed to parse stream {} ({}): {}",
                                stream.index,
                                stream.codec_name.as_deref().unwrap_or("unknown"),
                                e
                            );
                            return None;
                        }
                    }
                };

                if was_sampled {
                    debug!("Stream {} was analyzed using sampling mode", stream.index);
                }

                // Track per-stream intervals
                let mut stream_intervals: Vec<u64> = vec![0; num_intervals];

                // Aggregate this stream's contribution
                for (timestamp, size, _) in &frames {
                    let interval_idx = (*timestamp / interval_seconds).floor() as usize;
                    if interval_idx < num_intervals {
                        stream_intervals[interval_idx] += size;
                    }
                }

                Some((
                    stream.index,
                    stream.stream_type.clone(),
                    stream.codec_name.clone(),
                    stream_intervals,
                ))
            })
            .collect();

        // Check if cancelled during parallel processing
        if cancelled.load(Ordering::SeqCst) {
            info!("Job {} cancelled during stream analysis", job_id);
            return Err("Analysis cancelled".to_string());
        }

        let analyzed_count = analyzed_count.load(Ordering::SeqCst);

        // Combine results from parallel processing
        let mut combined_intervals: Vec<u64> = vec![0; num_intervals];
        let mut stream_data: Vec<(i32, StreamType, Option<String>, Vec<u64>)> = Vec::new();

        for (stream_index, stream_type, codec_name, intervals) in stream_results {
            // Add to combined intervals
            for (i, size) in intervals.iter().enumerate() {
                combined_intervals[i] += size;
            }
            stream_data.push((stream_index, stream_type, codec_name, intervals));
        }

        info!(
            "Parallel analysis complete: {}/{} streams analyzed in {:.2}s",
            analyzed_count,
            total_streams,
            analysis_start.elapsed().as_secs_f64()
        );

        // Emit progress update after parallel analysis completes
        emit_progress_enhanced(
            85,
            100,
            85.0,
            format!(
                "Stream analysis complete ({}/{} streams)",
                analyzed_count, total_streams
            ),
            Some(use_sampling),
            Some(total_streams),
            Some(total_streams),
        );

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
        emit_progress(90, 100, 90.0, "Aggregating bitrate data...".to_string());

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
        emit_progress_enhanced(
            95,
            100,
            95.0,
            "Calculating statistics...".to_string(),
            Some(use_sampling),
            Some(total_streams),
            Some(total_streams),
        );

        let statistics = calculate_statistics(&data_points);

        // Complete
        let total_elapsed = analysis_start.elapsed().as_secs_f64();
        emit_progress_enhanced(
            100,
            100,
            100.0,
            format!("Complete in {:.1}s", total_elapsed),
            Some(use_sampling),
            Some(total_streams),
            Some(total_streams),
        );
        info!(
            "Overall bitrate analysis complete in {:.2}s (sampling: {})",
            total_elapsed, use_sampling
        );

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
    .map_err(|e| format!("Task join error: {}", e));

    // Always clean up the job, regardless of success/failure
    debug!("Cleaning up job for {}", path_for_cleanup);
    jobs::complete_job(&path_for_cleanup);

    // Emit queue update after completion
    window
        .emit("job-queue-update", jobs::get_queue_status())
        .ok();

    // Note: Caching is now handled by the frontend
    // The frontend will save this result to the SQLite database

    // Now propagate error if there was one
    match &result {
        Ok(analysis) => info!(
            "Overall bitrate analysis completed successfully: {} ({} streams)",
            path,
            analysis
                .as_ref()
                .map(|a| a.stream_contributions.len())
                .unwrap_or(0)
        ),
        Err(e) => error!("Overall bitrate analysis failed for {}: {}", path, e),
    }

    result?
}

/// Cancel an ongoing bitrate analysis for a file
#[tauri::command]
pub async fn cancel_bitrate_analysis(path: String, window: tauri::Window) -> Result<bool, String> {
    info!("cancel_bitrate_analysis command: path={}", path);
    let result = jobs::cancel_job(&path);
    debug!("Cancel result: {}", result);
    // Emit queue update after cancellation
    window
        .emit("job-queue-update", jobs::get_queue_status())
        .ok();
    Ok(result)
}

/// Cancel all bitrate analysis jobs (queued and running)
#[tauri::command]
pub async fn cancel_all_bitrate_jobs(window: tauri::Window) -> Result<(), String> {
    info!("cancel_all_bitrate_jobs command called");
    jobs::cancel_all_jobs();
    // Emit queue update after cancelling all
    window
        .emit("job-queue-update", jobs::get_queue_status())
        .ok();
    Ok(())
}

/// Get the status of all active bitrate analysis jobs (legacy)
#[tauri::command]
pub async fn get_bitrate_job_status() -> Result<Vec<JobStatus>, String> {
    debug!("get_bitrate_job_status command called");
    let jobs = jobs::get_active_jobs();
    debug!("Active jobs: {}", jobs.len());
    Ok(jobs)
}

/// Get queue status (both queued and running jobs)
#[tauri::command]
pub async fn get_queue_status() -> Result<QueueStatus, String> {
    debug!("get_queue_status command called");
    let status = jobs::get_queue_status();
    debug!(
        "Queue status: {} queued, {} running, max {}",
        status.queued.len(),
        status.running.len(),
        status.max_parallel
    );
    Ok(status)
}

/// Set the maximum number of parallel jobs (1-8)
#[tauri::command]
pub async fn set_max_parallel_jobs(count: usize, window: tauri::Window) -> Result<(), String> {
    info!("set_max_parallel_jobs command: count={}", count);
    if count < 1 || count > 8 {
        warn!("Invalid max parallel jobs requested: {}", count);
        return Err("Max parallel jobs must be between 1 and 8".to_string());
    }
    jobs::set_max_parallel_jobs(count);
    // Emit queue update after changing limit (may start queued jobs)
    window
        .emit("job-queue-update", jobs::get_queue_status())
        .ok();
    Ok(())
}
