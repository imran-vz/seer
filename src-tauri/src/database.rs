//! Database migrations and configuration for SQLite
//!
//! This module defines the database schema and migrations for the Seer application.
//! The database is used for:
//! - Job tracking (background tasks like bitrate analysis, re-encoding, etc.)
//! - Caching (bitrate analysis results, media metadata, etc.)

use tauri_plugin_sql::{Migration, MigrationKind};

/// Get all database migrations
pub fn get_migrations() -> Vec<Migration> {
    vec![
        // Migration 1: Create jobs table
        Migration {
            version: 1,
            description: "create_jobs_table",
            sql: r#"
                CREATE TABLE IF NOT EXISTS jobs (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    job_type TEXT NOT NULL,
                    status TEXT NOT NULL DEFAULT 'pending',
                    file_path TEXT NOT NULL,
                    file_hash TEXT,
                    params TEXT,
                    result TEXT,
                    error_message TEXT,
                    progress INTEGER NOT NULL DEFAULT 0,
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    started_at TEXT,
                    completed_at TEXT
                );

                CREATE INDEX IF NOT EXISTS idx_jobs_status ON jobs(status);
                CREATE INDEX IF NOT EXISTS idx_jobs_file_path ON jobs(file_path);
                CREATE INDEX IF NOT EXISTS idx_jobs_job_type ON jobs(job_type);
                CREATE INDEX IF NOT EXISTS idx_jobs_created_at ON jobs(created_at);
            "#,
            kind: MigrationKind::Up,
        },
        // Migration 2: Create cache table
        Migration {
            version: 2,
            description: "create_cache_table",
            sql: r#"
                CREATE TABLE IF NOT EXISTS cache (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    cache_type TEXT NOT NULL,
                    cache_key TEXT NOT NULL UNIQUE,
                    file_path TEXT NOT NULL,
                    file_hash TEXT NOT NULL,
                    file_size INTEGER NOT NULL,
                    file_modified_at TEXT NOT NULL,
                    data TEXT NOT NULL,
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    expires_at TEXT
                );

                CREATE INDEX IF NOT EXISTS idx_cache_cache_key ON cache(cache_key);
                CREATE INDEX IF NOT EXISTS idx_cache_file_path ON cache(file_path);
                CREATE INDEX IF NOT EXISTS idx_cache_cache_type ON cache(cache_type);
                CREATE INDEX IF NOT EXISTS idx_cache_expires_at ON cache(expires_at);
            "#,
            kind: MigrationKind::Up,
        },
        // Migration 3: Create bitrate_analysis table for storing parsed bitrate data
        Migration {
            version: 3,
            description: "create_bitrate_analysis_table",
            sql: r#"
                CREATE TABLE IF NOT EXISTS bitrate_analysis (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    file_path TEXT NOT NULL,
                    file_hash TEXT NOT NULL,
                    file_size INTEGER NOT NULL,
                    duration REAL NOT NULL,
                    interval_seconds REAL NOT NULL,
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    UNIQUE(file_path, file_hash)
                );

                CREATE INDEX IF NOT EXISTS idx_bitrate_analysis_file_path ON bitrate_analysis(file_path);
                CREATE INDEX IF NOT EXISTS idx_bitrate_analysis_file_hash ON bitrate_analysis(file_hash);
            "#,
            kind: MigrationKind::Up,
        },
        // Migration 4: Create bitrate_data_points table for time-series bitrate data
        Migration {
            version: 4,
            description: "create_bitrate_data_points_table",
            sql: r#"
                CREATE TABLE IF NOT EXISTS bitrate_data_points (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    analysis_id INTEGER NOT NULL,
                    timestamp REAL NOT NULL,
                    bitrate INTEGER NOT NULL,
                    frame_type TEXT,
                    FOREIGN KEY (analysis_id) REFERENCES bitrate_analysis(id) ON DELETE CASCADE
                );

                CREATE INDEX IF NOT EXISTS idx_bitrate_data_points_analysis_id ON bitrate_data_points(analysis_id);
            "#,
            kind: MigrationKind::Up,
        },
        // Migration 5: Create bitrate_statistics table
        Migration {
            version: 5,
            description: "create_bitrate_statistics_table",
            sql: r#"
                CREATE TABLE IF NOT EXISTS bitrate_statistics (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    analysis_id INTEGER NOT NULL UNIQUE,
                    min_bitrate INTEGER NOT NULL,
                    max_bitrate INTEGER NOT NULL,
                    avg_bitrate INTEGER NOT NULL,
                    median_bitrate INTEGER NOT NULL,
                    std_deviation REAL NOT NULL,
                    total_frames INTEGER NOT NULL,
                    FOREIGN KEY (analysis_id) REFERENCES bitrate_analysis(id) ON DELETE CASCADE
                );
            "#,
            kind: MigrationKind::Up,
        },
        // Migration 6: Create peak_intervals table
        Migration {
            version: 6,
            description: "create_peak_intervals_table",
            sql: r#"
                CREATE TABLE IF NOT EXISTS peak_intervals (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    statistics_id INTEGER NOT NULL,
                    start_time REAL NOT NULL,
                    end_time REAL NOT NULL,
                    peak_bitrate INTEGER NOT NULL,
                    duration REAL NOT NULL,
                    FOREIGN KEY (statistics_id) REFERENCES bitrate_statistics(id) ON DELETE CASCADE
                );

                CREATE INDEX IF NOT EXISTS idx_peak_intervals_statistics_id ON peak_intervals(statistics_id);
            "#,
            kind: MigrationKind::Up,
        },
        // Migration 7: Create stream_contributions table
        Migration {
            version: 7,
            description: "create_stream_contributions_table",
            sql: r#"
                CREATE TABLE IF NOT EXISTS stream_contributions (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    analysis_id INTEGER NOT NULL,
                    stream_index INTEGER NOT NULL,
                    stream_type TEXT NOT NULL,
                    codec_name TEXT NOT NULL,
                    percentage REAL NOT NULL,
                    FOREIGN KEY (analysis_id) REFERENCES bitrate_analysis(id) ON DELETE CASCADE
                );

                CREATE INDEX IF NOT EXISTS idx_stream_contributions_analysis_id ON stream_contributions(analysis_id);
            "#,
            kind: MigrationKind::Up,
        },
        // Migration 8: Create stream_data_points table for per-stream bitrate data
        Migration {
            version: 8,
            description: "create_stream_data_points_table",
            sql: r#"
                CREATE TABLE IF NOT EXISTS stream_data_points (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    contribution_id INTEGER NOT NULL,
                    timestamp REAL NOT NULL,
                    bitrate INTEGER NOT NULL,
                    frame_type TEXT,
                    FOREIGN KEY (contribution_id) REFERENCES stream_contributions(id) ON DELETE CASCADE
                );

                CREATE INDEX IF NOT EXISTS idx_stream_data_points_contribution_id ON stream_data_points(contribution_id);
            "#,
            kind: MigrationKind::Up,
        },
        // Migration 9: Create settings table for app configuration
        Migration {
            version: 9,
            description: "create_settings_table",
            sql: r#"
                CREATE TABLE IF NOT EXISTS settings (
                    key TEXT PRIMARY KEY NOT NULL,
                    value TEXT NOT NULL,
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                );

                -- Insert default settings
                INSERT OR IGNORE INTO settings (key, value) VALUES ('default_download_location', '');
                INSERT OR IGNORE INTO settings (key, value) VALUES ('start_in_last_directory', 'true');
                INSERT OR IGNORE INTO settings (key, value) VALUES ('last_directory', '');
                INSERT OR IGNORE INTO settings (key, value) VALUES ('show_hidden_files', 'false');
                INSERT OR IGNORE INTO settings (key, value) VALUES ('confirm_before_delete', 'true');
                INSERT OR IGNORE INTO settings (key, value) VALUES ('use_trash_by_default', 'true');
                INSERT OR IGNORE INTO settings (key, value) VALUES ('enable_caching', 'true');
                INSERT OR IGNORE INTO settings (key, value) VALUES ('cache_expiration_days', '30');
            "#,
            kind: MigrationKind::Up,
        },
        // Migration 10: Add job queue support and max parallel jobs setting
        Migration {
            version: 10,
            description: "add_job_queue_support",
            sql: r#"
                -- Add queue position column for job ordering
                ALTER TABLE jobs ADD COLUMN queue_position INTEGER;

                -- Add index for efficient queue queries
                CREATE INDEX IF NOT EXISTS idx_jobs_queue_position ON jobs(queue_position);

                -- Add max parallel jobs setting
                INSERT OR IGNORE INTO settings (key, value) VALUES ('max_parallel_jobs', '2');
            "#,
            kind: MigrationKind::Up,
        },
    ]
}

/// Database name constant
pub const DATABASE_NAME: &str = "seer.db";

/// Get the full database URL for SQLite
pub fn get_database_url() -> String {
    format!("sqlite:{}", DATABASE_NAME)
}
