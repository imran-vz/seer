//! Rewrites a file with some streams left out or renamed. Packets are copied
//! as they are, so nothing is re-encoded and quality is untouched.

use crate::bitrate::Cancelled;
use anyhow::{Context, Result, bail};
use ff::ffi;
use ffmpeg_the_third as ff;
use std::collections::HashMap;
use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// What to change while copying a file. Everything not named is kept.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Edits {
    /// Indexes of streams to leave out.
    pub remove: Vec<usize>,
    /// New titles by stream index. `None` removes the title.
    pub titles: HashMap<usize, Option<String>>,
    /// A new title for the file as a whole. `Some(None)` removes it.
    pub container_title: Option<Option<String>>,
}

impl Edits {
    pub fn is_empty(&self) -> bool {
        self.remove.is_empty() && self.titles.is_empty() && self.container_title.is_none()
    }
}

/// Copies `input` to `output`, leaving out the streams whose indexes are in
/// `remove`. Container tags, stream tags, dispositions and chapters are kept.
/// On failure or cancellation the partial output is deleted.
pub fn copy_without(
    input: &Path,
    output: &Path,
    remove: &[usize],
    cancel: &AtomicBool,
    progress: impl FnMut(f32),
) -> Result<()> {
    let edits = Edits {
        remove: remove.to_vec(),
        ..Edits::default()
    };
    copy_with(input, output, &edits, cancel, progress)
}

/// Copies `input` to `output` with `edits` applied. On failure or
/// cancellation the partial output is deleted.
pub fn copy_with(
    input: &Path,
    output: &Path,
    edits: &Edits,
    cancel: &AtomicBool,
    progress: impl FnMut(f32),
) -> Result<()> {
    let result = copy(input, output, edits, cancel, progress);
    if result.is_err() {
        std::fs::remove_file(output).ok();
    }
    result
}

/// Sets or removes the "title" entry of a libav dictionary.
///
/// # Safety
/// `dict` must point to a dictionary pointer owned by an open context.
unsafe fn set_title(dict: *mut *mut ffi::AVDictionary, title: Option<&str>) -> Result<()> {
    let value = title
        .map(CString::new)
        .transpose()
        .context("a title cannot contain a NUL byte")?;
    // A null value deletes the entry.
    let value = value.as_ref().map_or(std::ptr::null(), |v| v.as_ptr());
    if unsafe { ffi::av_dict_set(dict, c"title".as_ptr(), value, 0) } < 0 {
        bail!("could not change a title");
    }
    Ok(())
}

fn copy(
    input: &Path,
    output: &Path,
    edits: &Edits,
    cancel: &AtomicBool,
    mut progress: impl FnMut(f32),
) -> Result<()> {
    if edits.is_empty() {
        bail!("there is nothing to change");
    }
    let remove = &edits.remove;

    let mut source =
        ff::format::input(input).with_context(|| format!("could not read {}", input.display()))?;
    let mut target = ff::format::output(output)
        .with_context(|| format!("could not create {}", output.display()))?;

    let duration = match source.duration() {
        d if d > 0 => d as f64 / ffi::AV_TIME_BASE as f64,
        _ => 0.0,
    };

    // Output index for each input stream that is kept.
    let mut mapping: Vec<Option<usize>> = vec![None; source.nb_streams() as usize];
    let mut kept = 0;
    for stream in source.streams() {
        if remove.contains(&stream.index()) {
            continue;
        }
        // SAFETY: both contexts are open and outlive these pointers; the new
        // stream is owned by `target`.
        unsafe {
            let from = stream.as_ptr();
            let to = ffi::avformat_new_stream(target.as_mut_ptr(), std::ptr::null());
            if to.is_null() {
                bail!("could not add a stream to the output");
            }
            if ffi::avcodec_parameters_copy((*to).codecpar, (*from).codecpar) < 0 {
                bail!("could not copy stream {} parameters", stream.index());
            }
            // Let the muxer pick the tag that suits the output container.
            (*(*to).codecpar).codec_tag = 0;
            (*to).time_base = (*from).time_base;
            (*to).avg_frame_rate = (*from).avg_frame_rate;
            (*to).sample_aspect_ratio = (*from).sample_aspect_ratio;
            (*to).disposition = (*from).disposition;
            ffi::av_dict_copy(&mut (*to).metadata, (*from).metadata, 0);
            if let Some(title) = edits.titles.get(&stream.index()) {
                set_title(&mut (*to).metadata, title.as_deref())?;
            }
        }
        mapping[stream.index()] = Some(kept);
        kept += 1;
    }
    if kept == 0 {
        bail!("a file must keep at least one stream");
    }
    if !remove.is_empty() && kept == mapping.len() {
        bail!("none of the chosen streams exist in this file");
    }

    // SAFETY: as above.
    unsafe {
        ffi::av_dict_copy(
            &mut (*target.as_mut_ptr()).metadata,
            (*source.as_ptr()).metadata,
            0,
        );
        if let Some(title) = &edits.container_title {
            set_title(&mut (*target.as_mut_ptr()).metadata, title.as_deref())?;
        }
    }
    let chapters: Vec<_> = source
        .chapters()
        .map(|c| {
            let title = c.metadata().get("title").unwrap_or("").to_owned();
            (c.id(), c.time_base(), c.start(), c.end(), title)
        })
        .collect();
    for (id, time_base, start, end, title) in chapters {
        target.add_chapter(id, time_base, start, end, &title)?;
    }

    target
        .write_header()
        .context("could not write the output header")?;
    // The muxer may have changed time bases while writing the header.
    let time_bases: Vec<_> = target.streams().map(|s| s.time_base()).collect();

    let mut count = 0u64;
    let mut last_report = -1.0f32;
    for item in source.packets() {
        let (stream, mut packet) = item.context("could not read a packet")?;
        let Some(index) = mapping.get(stream.index()).copied().flatten() else {
            continue;
        };
        let from = stream.time_base();

        count += 1;
        if count.is_multiple_of(256) {
            if cancel.load(Ordering::Relaxed) {
                bail!(Cancelled);
            }
            if let (true, Some(ts)) = (duration > 0.0, packet.dts().or(packet.pts())) {
                let fraction = (ts as f64 * f64::from(from) / duration).clamp(0.0, 1.0) as f32;
                if fraction - last_report >= 0.005 {
                    last_report = fraction;
                    progress(fraction);
                }
            }
        }

        packet.rescale_ts(from, time_bases[index]);
        packet.set_position(-1);
        packet.set_stream(index);
        packet
            .write_interleaved(&mut target)
            .context("could not write a packet")?;
    }
    target
        .write_trailer()
        .context("could not finish the output")?;
    progress(1.0);
    Ok(())
}

/// Applies `edits` to a file in place.
///
/// The new file is written beside the original and only swapped in once it
/// is complete. The original goes to the Trash, so the change can be undone.
pub fn rewrite(
    path: &Path,
    edits: &Edits,
    cancel: &AtomicBool,
    progress: impl FnMut(f32),
) -> Result<()> {
    let temporary = sibling(path);
    copy_with(path, &temporary, edits, cancel, progress)?;
    if let Err(e) = crate::fs::trash(&[path.to_owned()]) {
        std::fs::remove_file(&temporary).ok();
        return Err(e).context("could not move the original to the Trash");
    }
    std::fs::rename(&temporary, path).context("could not put the new file in place")
}

/// A hidden path next to `path` with the same extension, so FFmpeg picks the
/// same container.
fn sibling(path: &Path) -> PathBuf {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut name = format!(".{stem}.seer-partial");
    if let Some(ext) = path.extension() {
        name = format!("{name}.{}", ext.to_string_lossy());
    }
    path.with_file_name(name)
}
