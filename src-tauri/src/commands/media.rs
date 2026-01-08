//! Media-related Tauri commands
//!
//! This module contains all Tauri commands for media operations.

use log::{debug, info};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::Emitter;

use crate::bitrate::compute_file_hash;
use crate::jobs::{self, JobStartResult, JobType};
use crate::media;
use crate::types::{BulkStreamRemovalResult, MediaStreams, StreamRemovalOp, StreamRemovalResult};

#[tauri::command]
pub fn get_media_streams(path: String) -> Result<MediaStreams, String> {
    media::get_media_streams(path)
}

#[tauri::command]
pub async fn remove_streams(
    path: String,
    stream_indices: Vec<i32>,
    overwrite: bool,
    window: tauri::Window,
) -> Result<StreamRemovalResult, String> {
    let path_clone = path.clone();

    // Compute file hash for job ID
    let file_hash = compute_file_hash(&path_clone)?;

    // Enqueue job - starts immediately if slot available, otherwise queues
    let (job_id, cancelled) = match jobs::enqueue_job(
        &path_clone,
        &file_hash,
        JobType::StreamRemoval {
            stream_indices: stream_indices.clone(),
            overwrite,
        },
    ) {
        JobStartResult::Started(id) | JobStartResult::Queued(id) => {
            // Get cancellation flag from the job
            let cancel_flag = jobs::get_job_cancel_flag(&path_clone)
                .ok_or("Failed to get job cancellation flag")?;
            (id, cancel_flag)
        }
        JobStartResult::AlreadyExists(job_id) => {
            return Err(format!(
                "Stream removal already queued or in progress for this file (job {})",
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
    let result = tauri::async_runtime::spawn_blocking(move || {
        info!(
            "Starting stream removal: path={}, streams={:?}, job_id={}",
            path_clone, stream_indices, job_id
        );

        // Check for cancellation
        if cancelled.load(Ordering::SeqCst) {
            return Err("Stream removal cancelled".to_string());
        }

        // Perform the actual stream removal
        media::remove_streams(path_clone, stream_indices, overwrite)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?;

    // Clean up the job
    jobs::complete_job(&path_for_cleanup);

    // Emit queue update after completion
    window
        .emit("job-queue-update", jobs::get_queue_status())
        .ok();

    result
}

#[tauri::command]
pub async fn bulk_remove_streams(
    operations: Vec<StreamRemovalOp>,
    overwrite: bool,
    window: tauri::Window,
) -> Result<BulkStreamRemovalResult, String> {
    let mut job_ids = Vec::new();
    let mut errors = Vec::new();
    let mut jobs_queued = 0;

    // Collect jobs to spawn
    let mut jobs_to_spawn: Vec<(String, Vec<i32>, bool, Arc<std::sync::atomic::AtomicBool>)> =
        Vec::new();

    for op in operations {
        let path = op.path.clone();
        let stream_indices = op.stream_indices;

        // Compute file hash for job ID
        let file_hash = match compute_file_hash(&path) {
            Ok(hash) => hash,
            Err(e) => {
                errors.push(format!("{}: {}", path, e));
                continue;
            }
        };

        // Enqueue job - starts immediately if slot available, otherwise queues
        match jobs::enqueue_job(
            &path,
            &file_hash,
            JobType::StreamRemoval {
                stream_indices: stream_indices.clone(),
                overwrite,
            },
        ) {
            JobStartResult::Started(id) | JobStartResult::Queued(id) => {
                job_ids.push(id);
                jobs_queued += 1;

                // Get cancellation flag for the job
                if let Some(cancel_flag) = jobs::get_job_cancel_flag(&path) {
                    jobs_to_spawn.push((path, stream_indices, overwrite, cancel_flag));
                }
            }
            JobStartResult::AlreadyExists(job_id) => {
                errors.push(format!(
                    "{}: Stream removal already queued or in progress (job {})",
                    path, job_id
                ));
            }
        }
    }

    // Emit queue update
    window
        .emit("job-queue-update", jobs::get_queue_status())
        .ok();

    info!(
        "Bulk stream removal: queued {} jobs, {} errors",
        jobs_queued,
        errors.len()
    );

    // Spawn workers for all jobs
    for (path, stream_indices, overwrite, cancelled) in jobs_to_spawn {
        let window_clone = window.clone();
        let path_clone = path.clone();

        tauri::async_runtime::spawn(async move {
            // Wait until this job is actually running (not just queued)
            loop {
                if cancelled.load(Ordering::SeqCst) {
                    debug!("Job cancelled before starting: {}", path_clone);
                    jobs::complete_job(&path_clone);
                    window_clone
                        .emit("job-queue-update", jobs::get_queue_status())
                        .ok();
                    return;
                }

                // Check if this job is in the running state
                if let Some(job_type) = jobs::get_job_details(&path_clone) {
                    if matches!(job_type, JobType::StreamRemoval { .. }) {
                        break; // Job is running, proceed with work
                    }
                }

                // Wait a bit before checking again
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }

            debug!("Starting stream removal worker for: {}", path_clone);

            let path_for_work = path_clone.clone();
            let cancelled_clone = cancelled.clone();

            // Run the actual work in a blocking thread
            let result = tauri::async_runtime::spawn_blocking(move || {
                if cancelled_clone.load(Ordering::SeqCst) {
                    return Err("Stream removal cancelled".to_string());
                }

                info!(
                    "Executing stream removal: path={}, streams={:?}",
                    path_for_work, stream_indices
                );

                media::remove_streams(path_for_work, stream_indices, overwrite)
            })
            .await;

            // Complete the job
            jobs::complete_job(&path_clone);

            // Emit queue update
            window_clone
                .emit("job-queue-update", jobs::get_queue_status())
                .ok();

            // Log result
            match result {
                Ok(Ok(_)) => {
                    info!("Stream removal completed successfully: {}", path_clone);
                }
                Ok(Err(e)) => {
                    info!("Stream removal failed for {}: {}", path_clone, e);
                }
                Err(e) => {
                    info!("Stream removal task failed for {}: {}", path_clone, e);
                }
            }
        });
    }

    Ok(BulkStreamRemovalResult {
        jobs_queued,
        job_ids,
        errors,
    })
}
