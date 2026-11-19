//! Reads container and stream information with libavformat. No decoding.

use anyhow::{Context, Result};
use ff::ffi;
use ff::format::stream::Disposition;
use ff::media::Type;
use ffmpeg_the_third as ff;
use serde::{Deserialize, Serialize};
use std::ffi::CStr;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StreamKind {
    Video,
    Audio,
    Subtitle,
    Attachment,
    Data,
}

impl StreamKind {
    pub fn label(self) -> &'static str {
        match self {
            StreamKind::Video => "Video",
            StreamKind::Audio => "Audio",
            StreamKind::Subtitle => "Subtitle",
            StreamKind::Attachment => "Attachment",
            StreamKind::Data => "Data",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct StreamFlags {
    pub default: bool,
    pub forced: bool,
    pub hearing_impaired: bool,
    pub visual_impaired: bool,
    pub commentary: bool,
    pub original: bool,
    pub dub: bool,
    /// Cover art stored as a video stream.
    pub attached_picture: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stream {
    pub index: usize,
    pub kind: StreamKind,
    /// Short codec name, e.g. "hevc".
    pub codec: String,
    pub codec_description: Option<String>,
    pub profile: Option<String>,
    pub language: Option<String>,
    pub title: Option<String>,
    pub flags: StreamFlags,
    /// Bits per second as declared by the container, when it declares one.
    pub bit_rate: Option<u64>,
    pub duration: Option<f64>,
    pub tags: Vec<(String, String)>,

    // Video
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub frame_rate: Option<f64>,
    pub pixel_format: Option<String>,
    pub bit_depth: Option<u32>,
    pub color_space: Option<String>,
    pub color_transfer: Option<String>,
    pub color_primaries: Option<String>,

    // Audio
    pub sample_rate: Option<u32>,
    pub channels: Option<u32>,
    pub channel_layout: Option<String>,
}

impl Stream {
    /// "HDR10", "HLG" or "Dolby Vision"-style label from the transfer curve.
    pub fn dynamic_range(&self) -> Option<&'static str> {
        match self.color_transfer.as_deref()? {
            "smpte2084" => Some("HDR"),
            "arib-std-b67" => Some("HLG"),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Chapter {
    pub start: f64,
    pub end: f64,
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaInfo {
    /// Short demuxer name, e.g. "matroska,webm".
    pub container: String,
    pub container_description: String,
    /// Seconds. Zero when the container does not know.
    pub duration: f64,
    /// Overall bits per second. Zero when unknown.
    pub bit_rate: u64,
    pub tags: Vec<(String, String)>,
    pub streams: Vec<Stream>,
    pub chapters: Vec<Chapter>,
}

impl MediaInfo {
    pub fn streams_of(&self, kind: StreamKind) -> impl Iterator<Item = &Stream> {
        self.streams.iter().filter(move |s| s.kind == kind)
    }

    /// The main picture stream, ignoring cover art.
    pub fn primary_video(&self) -> Option<&Stream> {
        self.streams_of(StreamKind::Video)
            .find(|s| !s.flags.attached_picture)
    }

    pub fn primary_audio(&self) -> Option<&Stream> {
        let mut audio = self.streams_of(StreamKind::Audio);
        let first = audio.next()?;
        Some(
            self.streams_of(StreamKind::Audio)
                .find(|s| s.flags.default)
                .unwrap_or(first),
        )
    }
}

pub fn probe(path: &Path) -> Result<MediaInfo> {
    let input =
        ff::format::input(path).with_context(|| format!("could not read {}", path.display()))?;

    let duration = match input.duration() {
        d if d > 0 => d as f64 / ffi::AV_TIME_BASE as f64,
        _ => 0.0,
    };

    let streams = input.streams().map(|s| read_stream(&s)).collect();
    let chapters = input
        .chapters()
        .map(|c| {
            let tb = f64::from(c.time_base());
            Chapter {
                start: c.start() as f64 * tb,
                end: c.end() as f64 * tb,
                title: c.metadata().get("title").map(str::to_owned),
            }
        })
        .collect();

    Ok(MediaInfo {
        container: input.format().name().to_owned(),
        container_description: input.format().description().to_owned(),
        duration,
        bit_rate: input.bit_rate().max(0) as u64,
        tags: tags(input.metadata().iter()),
        streams,
        chapters,
    })
}

fn read_stream(stream: &ff::Stream) -> Stream {
    let params = stream.parameters();
    let disposition = stream.disposition();
    let meta = stream.metadata();
    let id = params.id();
    // SAFETY: the parameters pointer is valid for the life of `stream`.
    let raw = unsafe { &*params.as_ptr() };

    let kind = match params.medium() {
        Type::Video => StreamKind::Video,
        Type::Audio => StreamKind::Audio,
        Type::Subtitle => StreamKind::Subtitle,
        Type::Attachment => StreamKind::Attachment,
        _ => StreamKind::Data,
    };

    let descriptor = unsafe { ffi::avcodec_descriptor_get(raw.codec_id).as_ref() };
    let codec_description = descriptor.and_then(|d| c_string(d.long_name));
    let profile = c_string(unsafe { ffi::avcodec_profile_name(raw.codec_id, raw.profile) });

    let tb = f64::from(stream.time_base());
    let duration = (stream.duration() > 0).then(|| stream.duration() as f64 * tb);
    let bit_rate = match params.bit_rate() {
        b if b > 0 => Some(b as u64),
        // Matroska keeps per-stream statistics in tags.
        _ => meta.get("BPS").and_then(|v| v.parse().ok()),
    };

    let mut out = Stream {
        index: stream.index(),
        kind,
        codec: id.name().to_owned(),
        codec_description,
        profile,
        language: meta
            .get("language")
            .filter(|l| *l != "und")
            .map(str::to_owned),
        title: meta.get("title").map(str::to_owned),
        flags: StreamFlags {
            default: disposition.contains(Disposition::DEFAULT),
            forced: disposition.contains(Disposition::FORCED),
            hearing_impaired: disposition.contains(Disposition::HEARING_IMPAIRED),
            visual_impaired: disposition.contains(Disposition::VISUAL_IMPAIRED),
            commentary: disposition.contains(Disposition::COMMENT),
            original: disposition.contains(Disposition::ORIGINAL),
            dub: disposition.contains(Disposition::DUB),
            attached_picture: disposition.contains(Disposition::ATTACHED_PIC),
        },
        bit_rate,
        duration,
        tags: tags(meta.iter()),
        width: None,
        height: None,
        frame_rate: None,
        pixel_format: None,
        bit_depth: None,
        color_space: None,
        color_transfer: None,
        color_primaries: None,
        sample_rate: None,
        channels: None,
        channel_layout: None,
    };

    match kind {
        StreamKind::Video => {
            out.width = Some(params.width()).filter(|w| *w > 0);
            out.height = Some(params.height()).filter(|h| *h > 0);
            let rate = stream.avg_frame_rate();
            if rate.numerator() > 0 && rate.denominator() > 0 && !out.flags.attached_picture {
                out.frame_rate = Some(f64::from(rate));
            }
            // SAFETY: for video streams `format` is an AVPixelFormat value;
            // libav returns null for unknown values.
            unsafe {
                let format: ffi::AVPixelFormat = std::mem::transmute(raw.format);
                out.pixel_format = c_string(ffi::av_get_pix_fmt_name(format));
                if let Some(desc) = ffi::av_pix_fmt_desc_get(format).as_ref() {
                    out.bit_depth = Some(desc.comp[0].depth as u32).filter(|d| *d > 0);
                }
            }
            out.color_space = params.color_space().name().map(str::to_owned);
            out.color_transfer = params
                .color_transfer_characteristic()
                .name()
                .map(str::to_owned);
            out.color_primaries = params.color_primaries().name().map(str::to_owned);
            for unknown in [
                &mut out.color_space,
                &mut out.color_transfer,
                &mut out.color_primaries,
            ] {
                if unknown.as_deref() == Some("unknown") {
                    *unknown = None;
                }
            }
        }
        StreamKind::Audio => {
            out.sample_rate = Some(params.sample_rate()).filter(|r| *r > 0);
            let layout = params.ch_layout();
            out.channels = Some(layout.channels()).filter(|c| *c > 0);
            out.channel_layout = Some(layout.description()).filter(|d| !d.is_empty());
            out.bit_depth = Some(params.bits_per_raw_sample()).filter(|d| *d > 0);
        }
        _ => {}
    }
    out
}

fn tags<'a>(iter: impl Iterator<Item = (&'a str, &'a str)>) -> Vec<(String, String)> {
    iter.map(|(k, v)| (k.to_owned(), v.to_owned())).collect()
}

fn c_string(ptr: *const std::ffi::c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    // SAFETY: libav returns static, NUL-terminated strings here.
    Some(
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned(),
    )
}
