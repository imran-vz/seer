//! Centralized job queue for all async operations
//!
//! Handles bitrate analysis, stream removal, and other long-running tasks.
//! Jobs identified by file hash (SHA256) with configurable parallel execution.

use dashmap::DashMap;
use log::{debug, info, warn};
use once_cell::sync::Lazy;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::types::{JobInfo, QueueStatus};

/// Progress information for a job
#[derive(Debug, Clone)]
pub struct JobProgress {
    pub current: usize,
    pub total: usize,
    pub percentage: f64,
    pub stage: String,
}

impl Default for JobProgress {
    fn default() -> Self {
        Self {
            current: 0,
            total: 100,
            percentage: 0.0,
            stage: "Starting...".to_string(),
        }
    }
}

/// Type of job operation
#[derive(Debug, Clone)]
pub enum JobType {
    BitrateAnalysis,
    StreamRemoval {
        stream_indices: Vec<i32>,
        overwrite: bool,
    },
    DependencyInstallation {
        tool: String,
        method: String,
    },
}

impl JobType {
    pub fn name(&self) -> &str {
        match self {
            JobType::BitrateAnalysis => "bitrate_analysis",
            JobType::StreamRemoval { .. } => "stream_removal",
            JobType::DependencyInstallation { .. } => "dependency_installation",
        }
    }
}

/// Represents a job
#[derive(Debug, Clone)]
pub struct Job {
    pub id: String, // File hash (SHA256)
    pub path: String,
    pub job_type: JobType,
    pub cancelled: Arc<AtomicBool>,
    pub queued_at: std::time::Instant,
    pub started_at: Option<std::time::Instant>,
    pub progress: Arc<Mutex<JobProgress>>,
}

impl Job {
    fn new(path: String, file_hash: String, job_type: JobType) -> Self {
        Self {
            id: file_hash,
            path,
            job_type,
            cancelled: Arc::new(AtomicBool::new(false)),
            queued_at: std::time::Instant::now(),
            started_at: None,
            progress: Arc::new(Mutex::new(JobProgress::default())),
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

    pub fn update_progress(&self, progress: JobProgress) {
        if let Ok(mut p) = self.progress.lock() {
            *p = progress;
        }
    }

    pub fn get_progress(&self) -> JobProgress {
        self.progress.lock().map(|p| p.clone()).unwrap_or_default()
    }
}

/// Job queue manager
pub struct JobQueue {
    /// Queued jobs waiting for execution slot
    queued: Arc<Mutex<VecDeque<Job>>>,
    /// Currently running jobs (keyed by file path)
    running: Arc<DashMap<String, Job>>,
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
/// Default to 4 parallel jobs for better multi-core utilization
static JOB_QUEUE: Lazy<JobQueue> = Lazy::new(|| JobQueue::new(4));

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

/// Enqueue a new job
/// Uses file hash as job ID. If slots available, starts immediately. Otherwise queues.
pub fn enqueue_job(path: &str, file_hash: &str, job_type: JobType) -> JobStartResult {
    debug!(
        "enqueue_job: path={}, file_hash={}, job_type={}",
        path,
        file_hash,
        job_type.name()
    );

    // Check if already queued or running (do this BEFORE taking any locks)
    if JOB_QUEUE.is_queued_or_running(path) {
        debug!("Job already queued or running for path: {}", path);
        // Find the existing job ID
        if let Some(existing) = JOB_QUEUE.running.get(path) {
            warn!(
                "Job {} ({}) already exists for file: {}",
                existing.id,
                existing.job_type.name(),
                path
            );
            return JobStartResult::AlreadyExists(existing.id.clone());
        }
        // Check queued jobs
        let queued = JOB_QUEUE.queued.lock().unwrap();
        if let Some(existing) = queued.iter().find(|j| j.path == path) {
            warn!(
                "Job {} ({}) already queued for file: {}",
                existing.id,
                existing.job_type.name(),
                path
            );
            return JobStartResult::AlreadyExists(existing.id.clone());
        }
    }

    let mut job = Job::new(path.to_string(), file_hash.to_string(), job_type.clone());
    let job_id = job.id.clone();
    let max = JOB_QUEUE.max_parallel.load(Ordering::SeqCst);
    debug!("Max parallel jobs: {}", max);

    // Count running jobs BEFORE taking the entry lock to avoid deadlock
    // (DashMap entry() locks the shard, and iter() would try to lock all shards)
    let current_running = JOB_QUEUE.running_count();
    debug!(
        "Current running jobs: {}/{} (capacity available: {})",
        current_running,
        max,
        current_running < max
    );

    // Use DashMap's entry API for atomic check-and-insert
    use dashmap::mapref::entry::Entry;

    match JOB_QUEUE.running.entry(path.to_string()) {
        Entry::Vacant(entry) => {
            if current_running < max {
                // We have capacity, start immediately
                job.start();
                entry.insert(job);
                info!(
                    "Started {} job {} for file: {} [running: {}/{}]",
                    job_type.name(),
                    job_id,
                    path,
                    current_running + 1,
                    max
                );
                JobStartResult::Started(job_id)
            } else {
                // No capacity, queue for later
                drop(entry); // Release the entry before locking queued
                let mut queued = JOB_QUEUE.queued.lock().unwrap();
                queued.push_back(job);
                info!(
                    "Queued {} job {} for file: {} (queue position: {})",
                    job_type.name(),
                    job_id,
                    path,
                    queued.len()
                );
                JobStartResult::Queued(job_id)
            }
        }
        Entry::Occupied(entry) => {
            // Job already running for this file
            warn!(
                "Job {} ({}) already running for file: {}",
                entry.get().id,
                entry.get().job_type.name(),
                path
            );
            JobStartResult::AlreadyExists(entry.get().id.clone())
        }
    }
}

/// Try to start next queued job if slots available
/// Called after job completion or max_parallel change
pub fn try_start_next_job() {
    let max = JOB_QUEUE.max_parallel.load(Ordering::SeqCst);
    let running_count = JOB_QUEUE.running_count();

    debug!(
        "try_start_next_job: running={}/{}, checking for queued jobs",
        running_count, max
    );

    while JOB_QUEUE.running_count() < max {
        let mut queued = JOB_QUEUE.queued.lock().unwrap();
        let queued_count = queued.len();

        if let Some(mut job) = queued.pop_front() {
            let job_id = job.id.clone();
            let path = job.path.clone();
            let job_type_name = job.job_type.name().to_string();
            job.start();
            drop(queued); // Release lock before inserting
            JOB_QUEUE.running.insert(path.clone(), job);
            info!(
                "Started queued {} job {} for file: {} [{} remaining in queue]",
                job_type_name,
                job_id,
                path,
                queued_count - 1
            );
        } else {
            debug!("No more queued jobs to start");
            break;
        }
    }
}

/// Complete and remove a job, then try to start next queued job
pub fn complete_job(path: &str) {
    debug!("complete_job called for path: {}", path);

    if let Some((_, job)) = JOB_QUEUE.running.remove(path) {
        if let Some(started_at) = job.started_at {
            let elapsed = started_at.elapsed();
            info!(
                "Completed {} job {} for file: {} (took {:.2}s)",
                job.job_type.name(),
                job.id,
                path,
                elapsed.as_secs_f64()
            );
        } else {
            debug!("Job completed but never started: {}", path);
        }
    } else {
        debug!("Tried to complete non-existent job: {}", path);
    }

    // Try to start next queued job
    try_start_next_job();
}

/// Cancel a job (queued or running)
pub fn cancel_job(path: &str) -> bool {
    debug!("cancel_job called for path: {}", path);

    // Check if it's running
    if let Some(job) = JOB_QUEUE.running.get(path) {
        job.cancel();
        info!(
            "Cancelled running {} job {} for file: {}",
            job.job_type.name(),
            job.id,
            path
        );
        return true;
    }

    // Check if it's queued
    let mut queued = JOB_QUEUE.queued.lock().unwrap();
    if let Some(pos) = queued.iter().position(|j| j.path == path) {
        let job = queued.remove(pos).unwrap();
        info!(
            "Cancelled queued {} job {} for file: {}",
            job.job_type.name(),
            job.id,
            path
        );
        return true;
    }

    debug!("No job found to cancel for path: {}", path);
    false
}

/// Cancel all jobs (queued and running)
pub fn cancel_all_jobs() {
    debug!("cancel_all_jobs called");

    // Cancel all running jobs
    let running_count = JOB_QUEUE.running.len();
    for entry in JOB_QUEUE.running.iter() {
        entry.cancel();
        debug!(
            "Cancelling running job {} ({}) for: {}",
            entry.id,
            entry.job_type.name(),
            entry.path
        );
    }
    info!("Cancelled {} running jobs", running_count);

    // Clear queued jobs
    let mut queued = JOB_QUEUE.queued.lock().unwrap();
    let queued_count = queued.len();
    if queued_count > 0 {
        for job in queued.iter() {
            debug!(
                "Clearing queued job {} ({}) for: {}",
                job.id,
                job.job_type.name(),
                job.path
            );
        }
    }
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
        let progress = job.get_progress();
        queued_jobs.push(JobInfo {
            job_id: 0, // Not used anymore, kept for compatibility
            path: job.path.clone(),
            state: format!("queued:{}", job.job_type.name()),
            queued_seconds: Some(job.queued_at.elapsed().as_secs_f64()),
            running_seconds: None,
            progress_current: Some(progress.current),
            progress_total: Some(progress.total),
            progress_percentage: Some(progress.percentage),
            progress_stage: Some(progress.stage),
        });
    }

    // Get running jobs
    for entry in JOB_QUEUE.running.iter() {
        if !entry.is_cancelled() {
            let progress = entry.get_progress();
            running_jobs.push(JobInfo {
                job_id: 0, // Not used anymore, kept for compatibility
                path: entry.path.clone(),
                state: format!("running:{}", entry.job_type.name()),
                queued_seconds: Some(entry.queued_at.elapsed().as_secs_f64()),
                running_seconds: entry
                    .started_at
                    .map(|started| started.elapsed().as_secs_f64()),
                progress_current: Some(progress.current),
                progress_total: Some(progress.total),
                progress_percentage: Some(progress.percentage),
                progress_stage: Some(progress.stage),
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
    let previous = JOB_QUEUE.max_parallel.load(Ordering::SeqCst);
    let clamped = count.clamp(1, 8);
    JOB_QUEUE.max_parallel.store(clamped, Ordering::SeqCst);
    info!(
        "Set max parallel jobs from {} to {} (requested: {})",
        previous, clamped, count
    );
    // Try to start queued jobs if limit increased
    if clamped > previous {
        debug!("Limit increased, attempting to start queued jobs");
        try_start_next_job();
    }
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

/// Update job progress by path
pub fn update_job_progress(path: &str, progress: JobProgress) {
    if let Some(job) = JOB_QUEUE.running.get(path) {
        job.update_progress(progress);
    }
}

/// Get job details by path
pub fn get_job_details(path: &str) -> Option<JobType> {
    if let Some(job) = JOB_QUEUE.running.get(path) {
        return Some(job.job_type.clone());
    }
    None
}

/// Get the status of all active jobs (legacy compatibility)
pub fn get_active_jobs() -> Vec<crate::types::JobStatus> {
    JOB_QUEUE
        .running
        .iter()
        .filter(|entry| !entry.is_cancelled())
        .map(|entry| crate::types::JobStatus {
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
    JOB_QUEUE
        .running
        .iter()
        .filter(|e| !e.is_cancelled())
        .count()
}
