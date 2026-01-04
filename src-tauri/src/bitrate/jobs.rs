//! Job queue management for bitrate analysis tasks
//!
//! Implements an in-memory job queue with configurable parallel execution.
//! Jobs are identified by file hash (SHA256) and queue when max parallel limit is reached.
//! Frontend manages SQLite persistence via jobStore.

use dashmap::DashMap;
use log::info;
use once_cell::sync::Lazy;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::types::{JobInfo, JobStatus, QueueStatus};

/// Represents a bitrate analysis job
#[derive(Debug, Clone)]
pub struct BitrateJob {
    pub id: String,  // File hash (SHA256)
    pub path: String,
    pub cancelled: Arc<AtomicBool>,
    pub queued_at: std::time::Instant,
    pub started_at: Option<std::time::Instant>,
}

impl BitrateJob {
    fn new(path: String, file_hash: String) -> Self {
        Self {
            id: file_hash,
            path,
            cancelled: Arc::new(AtomicBool::new(false)),
            queued_at: std::time::Instant::now(),
            started_at: None,
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    fn start(&mut self) {
        self.started_at = Some(std::time::Instant::now());
    }
}

/// Job queue manager
pub struct JobQueue {
    /// Queued jobs waiting for execution slot
    queued: Arc<Mutex<VecDeque<BitrateJob>>>,
    /// Currently running jobs (keyed by file path)
    running: Arc<DashMap<String, BitrateJob>>,
    /// Maximum number of parallel jobs
    max_parallel: Arc<AtomicUsize>,
}

impl JobQueue {
    fn new(max_parallel: usize) -> Self {
        Self {
            queued: Arc::new(Mutex::new(VecDeque::new())),
            running: Arc::new(DashMap::new()),
            max_parallel: Arc::new(AtomicUsize::new(max_parallel)),
        }
    }

    fn running_count(&self) -> usize {
        self.running.iter().filter(|e| !e.is_cancelled()).count()
    }

    fn is_queued_or_running(&self, path: &str) -> bool {
        if self.running.contains_key(path) {
            return true;
        }
        let queued = self.queued.lock().unwrap();
        queued.iter().any(|job| job.path == path)
    }
}

/// Global job queue instance
static JOB_QUEUE: Lazy<JobQueue> = Lazy::new(|| JobQueue::new(2));

/// Result of trying to enqueue a new job
#[derive(Debug)]
pub enum JobStartResult {
    /// New job started immediately
    Started(String),
    /// Job queued - waiting for execution slot
    Queued(String),
    /// A job is already queued or running for this file
    AlreadyExists(String),
}

/// Enqueue a new bitrate analysis job
/// Uses file hash as job ID. If slots available, starts immediately. Otherwise queues.
pub fn enqueue_job(path: &str, file_hash: &str) -> JobStartResult {
    // Check if already queued or running
    if JOB_QUEUE.is_queued_or_running(path) {
        // Find the existing job ID
        if let Some(existing) = JOB_QUEUE.running.get(path) {
            info!("Job {} already exists for file: {}", existing.id, path);
            return JobStartResult::AlreadyExists(existing.id.clone());
        }
        // Check queued jobs
        let queued = JOB_QUEUE.queued.lock().unwrap();
        if let Some(existing) = queued.iter().find(|j| j.path == path) {
            info!("Job {} already queued for file: {}", existing.id, path);
            return JobStartResult::AlreadyExists(existing.id.clone());
        }
    }

    let mut job = BitrateJob::new(path.to_string(), file_hash.to_string());
    let job_id = job.id.clone();
    let max = JOB_QUEUE.max_parallel.load(Ordering::SeqCst);

    // If we have capacity, start immediately
    if JOB_QUEUE.running_count() < max {
        job.start();
        JOB_QUEUE.running.insert(path.to_string(), job);
        info!("Started bitrate job {} for file: {}", job_id, path);
        JobStartResult::Started(job_id)
    } else {
        // Queue for later
        let mut queued = JOB_QUEUE.queued.lock().unwrap();
        queued.push_back(job);
        info!(
            "Queued bitrate job {} for file: {} (position: {})",
            job_id,
            path,
            queued.len()
        );
        JobStartResult::Queued(job_id)
    }
}

/// Try to start next queued job if slots available
/// Called after job completion or max_parallel change
pub fn try_start_next_job() {
    let max = JOB_QUEUE.max_parallel.load(Ordering::SeqCst);

    while JOB_QUEUE.running_count() < max {
        let mut queued = JOB_QUEUE.queued.lock().unwrap();
        if let Some(mut job) = queued.pop_front() {
            let job_id = job.id.clone();
            let path = job.path.clone();
            job.start();
            drop(queued); // Release lock before inserting
            JOB_QUEUE.running.insert(path.clone(), job);
            info!("Started queued job {} for file: {}", job_id, path);
        } else {
            break;
        }
    }
}

/// Complete and remove a bitrate job, then try to start next queued job
pub fn complete_job(path: &str, _file_hash: &str) {
    if let Some((_, job)) = JOB_QUEUE.running.remove(path) {
        if let Some(started_at) = job.started_at {
            let elapsed = started_at.elapsed();
            info!(
                "Completed bitrate job {} for file: {} (took {:.2}s)",
                job.id,
                path,
                elapsed.as_secs_f64()
            );
        }
    }
    // Try to start next queued job
    try_start_next_job();
}

/// Cancel a bitrate job (queued or running)
pub fn cancel_job(path: &str) -> bool {
    // Check if it's running
    if let Some(job) = JOB_QUEUE.running.get(path) {
        job.cancel();
        info!("Cancelled running bitrate job {} for file: {}", job.id, path);
        return true;
    }

    // Check if it's queued
    let mut queued = JOB_QUEUE.queued.lock().unwrap();
    if let Some(pos) = queued.iter().position(|j| j.path == path) {
        let job = queued.remove(pos).unwrap();
        info!("Cancelled queued bitrate job {} for file: {}", job.id, path);
        return true;
    }

    false
}

/// Cancel all jobs (queued and running)
pub fn cancel_all_jobs() {
    // Cancel all running jobs
    for entry in JOB_QUEUE.running.iter() {
        entry.cancel();
    }
    info!("Cancelled {} running jobs", JOB_QUEUE.running.len());

    // Clear queued jobs
    let mut queued = JOB_QUEUE.queued.lock().unwrap();
    let queued_count = queued.len();
    queued.clear();
    info!("Cleared {} queued jobs", queued_count);
}

/// Get queue status (both queued and running jobs)
pub fn get_queue_status() -> QueueStatus {
    let mut queued_jobs = Vec::new();
    let mut running_jobs = Vec::new();

    // Get queued jobs
    let queued = JOB_QUEUE.queued.lock().unwrap();
    for job in queued.iter() {
        queued_jobs.push(JobInfo {
            job_id: 0, // Not used anymore, kept for compatibility
            path: job.path.clone(),
            state: "queued".to_string(),
            queued_seconds: Some(job.queued_at.elapsed().as_secs_f64()),
            running_seconds: None,
        });
    }

    // Get running jobs
    for entry in JOB_QUEUE.running.iter() {
        if !entry.is_cancelled() {
            running_jobs.push(JobInfo {
                job_id: 0, // Not used anymore, kept for compatibility
                path: entry.path.clone(),
                state: "running".to_string(),
                queued_seconds: Some(entry.queued_at.elapsed().as_secs_f64()),
                running_seconds: entry
                    .started_at
                    .map(|started| started.elapsed().as_secs_f64()),
            });
        }
    }

    QueueStatus {
        queued: queued_jobs,
        running: running_jobs,
        max_parallel: JOB_QUEUE.max_parallel.load(Ordering::SeqCst),
    }
}

/// Set max parallel jobs (1-8)
pub fn set_max_parallel_jobs(count: usize) {
    let clamped = count.clamp(1, 8);
    JOB_QUEUE.max_parallel.store(clamped, Ordering::SeqCst);
    info!("Set max parallel jobs to {}", clamped);
    // Try to start queued jobs if limit increased
    try_start_next_job();
}

/// Get cancellation flag for a job by path (for commands to check cancellation)
pub fn get_job_cancel_flag(path: &str) -> Option<Arc<AtomicBool>> {
    if let Some(job) = JOB_QUEUE.running.get(path) {
        return Some(job.cancelled.clone());
    }

    // Check queued jobs too
    let queued = JOB_QUEUE.queued.lock().unwrap();
    queued
        .iter()
        .find(|j| j.path == path)
        .map(|j| j.cancelled.clone())
}

/// Get the status of all active jobs (legacy compatibility)
pub fn get_active_jobs() -> Vec<JobStatus> {
    JOB_QUEUE
        .running
        .iter()
        .filter(|entry| !entry.is_cancelled())
        .map(|entry| JobStatus {
            job_id: 0, // Not used anymore, kept for compatibility
            path: entry.path.clone(),
            running_seconds: entry
                .started_at
                .map(|started| started.elapsed().as_secs_f64())
                .unwrap_or(0.0),
        })
        .collect()
}

/// Check if a job is running for a specific file
#[allow(dead_code)]
pub fn is_job_running(path: &str) -> bool {
    JOB_QUEUE
        .running
        .get(path)
        .map(|job| !job.is_cancelled())
        .unwrap_or(false)
}

/// Get the number of active jobs
#[allow(dead_code)]
pub fn active_job_count() -> usize {
    JOB_QUEUE.running.iter().filter(|e| !e.is_cancelled()).count()
}
