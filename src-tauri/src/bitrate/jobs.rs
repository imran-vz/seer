//! Job queue management for bitrate analysis tasks
//!
//! Ensures only one analysis runs per file at a time and provides
//! cancellation support.

use dashmap::DashMap;
use log::info;
use once_cell::sync::Lazy;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use crate::types::JobStatus;

/// Unique job ID counter
static JOB_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Global map of active analysis jobs per file path
/// Key: file path, Value: job info
static ACTIVE_JOBS: Lazy<DashMap<String, BitrateJob>> = Lazy::new(DashMap::new);

/// Represents a bitrate analysis job
#[derive(Debug, Clone)]
pub struct BitrateJob {
    pub id: u64,
    pub path: String,
    pub cancelled: Arc<AtomicBool>,
    pub started_at: std::time::Instant,
}

impl BitrateJob {
    fn new(path: String) -> Self {
        Self {
            id: JOB_ID_COUNTER.fetch_add(1, Ordering::SeqCst),
            path,
            cancelled: Arc::new(AtomicBool::new(false)),
            started_at: std::time::Instant::now(),
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }
}

/// Result of trying to start a new job
pub enum JobStartResult {
    /// New job started successfully
    Started(BitrateJob),
    /// A job is already running for this file
    AlreadyRunning(u64),
}

/// Try to start a new bitrate analysis job for a file
/// Returns AlreadyRunning if a job is already running for this file
pub fn start_job(path: &str) -> JobStartResult {
    // Check if there's already an active job for this file
    if let Some(existing) = ACTIVE_JOBS.get(path) {
        if !existing.is_cancelled() {
            info!("Job {} already running for file: {}", existing.id, path);
            return JobStartResult::AlreadyRunning(existing.id);
        }
        // If cancelled, we can replace it
    }

    let job = BitrateJob::new(path.to_string());
    let job_id = job.id;
    ACTIVE_JOBS.insert(path.to_string(), job.clone());
    info!("Started new bitrate job {} for file: {}", job_id, path);
    JobStartResult::Started(job)
}

/// Complete and remove a bitrate job
pub fn complete_job(path: &str, job_id: u64) {
    if let Some((_, job)) = ACTIVE_JOBS.remove(path) {
        let elapsed = job.started_at.elapsed();
        info!(
            "Completed bitrate job {} for file: {} (took {:.2}s)",
            job_id,
            path,
            elapsed.as_secs_f64()
        );
    }
}

/// Cancel a bitrate job for a file
pub fn cancel_job(path: &str) -> bool {
    if let Some(job) = ACTIVE_JOBS.get(path) {
        job.cancel();
        info!("Cancelled bitrate job {} for file: {}", job.id, path);
        true
    } else {
        false
    }
}

/// Get the status of all active jobs
pub fn get_active_jobs() -> Vec<JobStatus> {
    ACTIVE_JOBS
        .iter()
        .filter(|entry| !entry.is_cancelled())
        .map(|entry| JobStatus {
            job_id: entry.id,
            path: entry.path.clone(),
            running_seconds: entry.started_at.elapsed().as_secs_f64(),
        })
        .collect()
}

/// Check if a job is running for a specific file
#[allow(dead_code)]
pub fn is_job_running(path: &str) -> bool {
    ACTIVE_JOBS
        .get(path)
        .map(|job| !job.is_cancelled())
        .unwrap_or(false)
}

/// Get the number of active jobs
#[allow(dead_code)]
pub fn active_job_count() -> usize {
    ACTIVE_JOBS.iter().filter(|e| !e.is_cancelled()).count()
}
