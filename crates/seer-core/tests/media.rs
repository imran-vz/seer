//! End-to-end checks against a real file generated with the ffmpeg binary.
//! Skipped when ffmpeg is not installed.

use seer_core::probe::StreamKind;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicBool;

fn sample(dir: &Path) -> Option<PathBuf> {
    let out = dir.join("sample.mkv");
    let status = Command::new("ffmpeg")
        .args(["-v", "error", "-y"])
        .args([
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=640x360:rate=25:duration=6",
        ])
        .args(["-f", "lavfi", "-i", "sine=frequency=440:duration=6"])
        .args(["-f", "lavfi", "-i", "sine=frequency=880:duration=6"])
        .args(["-map", "0:v", "-map", "1:a", "-map", "2:a"])
        .args([
            "-c:v", "mpeg4", "-b:v", "800k", "-c:a", "aac", "-b:a", "96k",
        ])
        .args([
            "-metadata",
            "title=Sample",
            "-metadata:s:a:0",
            "language=eng",
        ])
        .args([
            "-metadata:s:a:1",
            "language=fra",
            "-metadata:s:a:1",
            "title=Commentary",
        ])
        .args(["-disposition:a:1", "comment"])
        .arg(&out)
        .status()
        .ok()?;
    status.success().then_some(out)
}

#[test]
fn probe_and_bitrate() {
    seer_core::init().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let Some(path) = sample(dir.path()) else {
        eprintln!("ffmpeg not available; skipping");
        return;
    };

    let info = seer_core::probe::probe(&path).unwrap();
    assert!(info.container.contains("matroska"));
    assert!(
        (info.duration - 6.0).abs() < 0.2,
        "duration {}",
        info.duration
    );
    assert_eq!(info.streams.len(), 3);
    assert!(
        info.tags
            .iter()
            .any(|(k, v)| k.eq_ignore_ascii_case("title") && v == "Sample")
    );

    let video = info.primary_video().unwrap();
    assert_eq!(video.codec, "mpeg4");
    assert_eq!((video.width, video.height), (Some(640), Some(360)));
    assert_eq!(video.frame_rate, Some(25.0));
    assert_eq!(video.pixel_format.as_deref(), Some("yuv420p"));

    let audio: Vec<_> = info.streams_of(StreamKind::Audio).collect();
    assert_eq!(audio.len(), 2);
    assert_eq!(audio[0].codec, "aac");
    assert_eq!(audio[0].language.as_deref(), Some("eng"));
    assert_eq!(audio[0].sample_rate, Some(44_100));
    assert_eq!(audio[0].channels, Some(1));
    assert_eq!(audio[1].title.as_deref(), Some("Commentary"));
    assert!(audio[1].flags.commentary);

    let mut reports = 0;
    let analysis =
        seer_core::bitrate::analyze(&path, &AtomicBool::new(false), |_| reports += 1).unwrap();
    assert!(reports >= 1);
    assert_eq!(analysis.streams.len(), 3);
    assert_eq!(
        analysis.streams[0].kind,
        StreamKind::Video,
        "video is the largest stream"
    );
    assert_eq!(
        analysis.overall.len(),
        (analysis.duration / analysis.interval).round() as usize
    );
    assert!(analysis.stats.min > 0, "no empty interval at the end");

    // Summed packet bytes must account for nearly the whole file.
    let bytes: u64 = analysis.streams.iter().map(|s| s.total_bytes).sum();
    let file = std::fs::metadata(&path).unwrap().len();
    assert!(bytes <= file && bytes > file * 9 / 10, "{bytes} of {file}");
    let expected = bytes as f64 * 8.0 / analysis.duration;
    assert!((analysis.stats.average as f64 - expected).abs() < 1.0);
    assert!(analysis.stats.max >= analysis.stats.average);
    assert!(analysis.stats.min <= analysis.stats.median);

    // Fingerprint and cache round trip of a real result.
    let fp = seer_core::fingerprint::fingerprint(&path).unwrap();
    let cache = seer_core::cache::Cache::in_memory().unwrap();
    cache.put(&path, "probe", &fp, &info).unwrap();
    assert_eq!(cache.get(&path, "probe", &fp), Some(info));
}

#[test]
fn cancel_stops_analysis() {
    seer_core::init().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let Some(path) = sample(dir.path()) else {
        return;
    };
    let err = seer_core::bitrate::analyze(&path, &AtomicBool::new(true), |_| {}).unwrap_err();
    assert!(
        err.downcast_ref::<seer_core::bitrate::Cancelled>()
            .is_some()
    );
}

#[test]
fn probe_rejects_non_media() {
    seer_core::init().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("notes.txt");
    std::fs::write(&path, "").unwrap();
    assert!(seer_core::probe::probe(&path).is_err());
}

#[test]
fn copy_without_streams() {
    seer_core::init().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let Some(path) = sample(dir.path()) else {
        return;
    };
    let out = dir.path().join("pruned.mkv");
    let cancel = AtomicBool::new(false);

    // Drop the commentary track (stream 2).
    seer_core::remux::copy_without(&path, &out, &[2], &cancel, |_| {}).unwrap();
    let before = seer_core::probe::probe(&path).unwrap();
    let after = seer_core::probe::probe(&out).unwrap();
    assert_eq!(after.streams.len(), 2);
    assert!((after.duration - before.duration).abs() < 0.1);
    assert!(
        after
            .tags
            .iter()
            .any(|(k, v)| k.eq_ignore_ascii_case("title") && v == "Sample")
    );
    let audio: Vec<_> = after.streams_of(StreamKind::Audio).collect();
    assert_eq!(audio.len(), 1);
    assert_eq!(audio[0].language.as_deref(), Some("eng"));
    assert_eq!(after.primary_video().unwrap().codec, "mpeg4");

    // The kept streams carry exactly the bytes they had before.
    let bytes = |p: &Path| {
        let a = seer_core::bitrate::analyze(p, &cancel, |_| {}).unwrap();
        let mut sizes: Vec<_> = a
            .streams
            .iter()
            .map(|s| (s.codec.clone(), s.language.clone(), s.total_bytes))
            .collect();
        sizes.sort();
        sizes
    };
    let mut original = bytes(&path);
    original.retain(|(_, language, _)| language.as_deref() != Some("fra"));
    assert_eq!(original, bytes(&out));

    // Refusals leave nothing behind.
    let none = dir.path().join("none.mkv");
    assert!(seer_core::remux::copy_without(&path, &none, &[0, 1, 2], &cancel, |_| {}).is_err());
    assert!(seer_core::remux::copy_without(&path, &none, &[9], &cancel, |_| {}).is_err());
    assert!(!none.exists());
}

#[test]
fn retitles_streams_without_touching_their_data() {
    use seer_core::remux::{Edits, copy_with};
    seer_core::init().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let Some(path) = sample(dir.path()) else {
        return;
    };
    let out = dir.path().join("retitled.mkv");
    let cancel = AtomicBool::new(false);

    let edits = Edits {
        remove: Vec::new(),
        // Name the video, and strip the name from the commentary track.
        titles: [(0, Some("360p MPEG-4".to_owned())), (2, None)].into(),
        container_title: Some(None),
    };
    copy_with(&path, &out, &edits, &cancel, |_| {}).unwrap();

    let before = seer_core::probe::probe(&path).unwrap();
    let after = seer_core::probe::probe(&out).unwrap();
    assert_eq!(after.streams.len(), 3);
    assert_eq!(after.streams[0].title.as_deref(), Some("360p MPEG-4"));
    assert_eq!(before.streams[2].title.as_deref(), Some("Commentary"));
    assert_eq!(after.streams[2].title, None);
    assert!(
        !after
            .tags
            .iter()
            .any(|(k, _)| k.eq_ignore_ascii_case("title"))
    );
    // Everything else about the streams survives.
    assert_eq!(after.streams[2].language.as_deref(), Some("fra"));
    assert!(after.streams[2].flags.commentary);

    let bytes = |p: &Path| {
        let a = seer_core::bitrate::analyze(p, &cancel, |_| {}).unwrap();
        let mut sizes: Vec<_> = a
            .streams
            .iter()
            .map(|s| (s.stream_index, s.total_bytes))
            .collect();
        sizes.sort();
        sizes
    };
    assert_eq!(bytes(&path), bytes(&out));

    // No edits is an error, not a silent copy.
    assert!(
        copy_with(
            &path,
            &dir.path().join("none.mkv"),
            &Edits::default(),
            &cancel,
            |_| {}
        )
        .is_err()
    );
}
