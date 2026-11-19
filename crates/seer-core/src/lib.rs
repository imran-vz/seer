//! Seer's engine: file listing, media probing, bitrate analysis and caching.
//!
//! Nothing here knows about the UI. Every function is synchronous and safe to
//! call from a background thread.

pub mod bitrate;
pub mod cache;
pub mod fingerprint;
pub mod format;
pub mod fs;
pub mod naming;
pub mod probe;
pub mod recommend;
pub mod remux;
pub mod titles;

/// Must be called once before any probing.
pub fn init() -> anyhow::Result<()> {
    ffmpeg_the_third::init()?;
    ffmpeg_the_third::util::log::set_level(ffmpeg_the_third::util::log::Level::Fatal);
    Ok(())
}
