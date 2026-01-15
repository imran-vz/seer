use log::{debug, warn};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config;
use crate::files;
use crate::media::{find_command, invalidate_probe_cache};
use crate::types::{
    is_image_extension, is_video_audio_extension, FileMetadata, MetadataAction, MetadataEntry,
    MetadataOperation, MetadataOrigin, MetadataScope, MetadataSnapshot, MetadataToolAvailability,
    MetadataUpdateResult, StreamSummary,
};

fn detect_tools() -> MetadataToolAvailability {
    MetadataToolAvailability {
        ffmpeg: find_command("ffmpeg").is_some(),
        ffprobe: find_command("ffprobe").is_some(),
        exiftool: find_command("exiftool").is_some(),
    }
}

fn parse_ffprobe_tags(
    ffprobe_data: Option<String>,
    tools: &MetadataToolAvailability,
) -> (Vec<MetadataEntry>, Vec<MetadataEntry>, Vec<StreamSummary>) {
    let mut format_tags = Vec::new();
    let mut stream_tags = Vec::new();
    let mut stream_summaries = Vec::new();

    if ffprobe_data.is_none() {
        return (format_tags, stream_tags, stream_summaries);
    }

    let parsed: Value = match serde_json::from_str(&ffprobe_data.unwrap_or_default()) {
        Ok(val) => val,
        Err(err) => {
            warn!("Failed to parse ffprobe data: {}", err);
            return (format_tags, stream_tags, stream_summaries);
        }
    };

    if let Some(format) = parsed.get("format") {
        if let Some(tags) = format.get("tags").and_then(|t| t.as_object()) {
            for (key, value) in tags.iter() {
                if let Some(val_str) = value.as_str() {
                    format_tags.push(MetadataEntry {
                        key: key.clone(),
                        value: val_str.to_string(),
                        scope: MetadataScope::Format,
                        stream_index: None,
                        origin: MetadataOrigin::Ffprobe,
                        editable: tools.ffmpeg,
                    });
                }
            }
        }

        let format_name = format
            .get("format_long_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                format
                    .get("format_name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            });
        let duration = format
            .get("duration")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok());
        let bit_rate = format
            .get("bit_rate")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        stream_summaries.push(StreamSummary {
            index: -1,
            codec_type: Some("format".to_string()),
            codec_name: format_name,
            codec_long_name: None,
            duration,
            bit_rate,
        });
    }

    if let Some(streams) = parsed.get("streams").and_then(|s| s.as_array()) {
        for stream in streams {
            let index = stream.get("index").and_then(|v| v.as_i64()).unwrap_or(-1) as i32;

            if let Some(tags) = stream.get("tags").and_then(|t| t.as_object()) {
                for (key, value) in tags.iter() {
                    if let Some(val_str) = value.as_str() {
                        stream_tags.push(MetadataEntry {
                            key: key.clone(),
                            value: val_str.to_string(),
                            scope: MetadataScope::Stream,
                            stream_index: Some(index),
                            origin: MetadataOrigin::Ffprobe,
                            editable: tools.ffmpeg,
                        });
                    }
                }
            }

            let codec_type = stream
                .get("codec_type")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let codec_name = stream
                .get("codec_name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let codec_long_name = stream
                .get("codec_long_name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let duration = stream
                .get("duration")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<f64>().ok());
            let bit_rate = stream
                .get("bit_rate")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            stream_summaries.push(StreamSummary {
                index,
                codec_type,
                codec_name,
                codec_long_name,
                duration,
                bit_rate,
            });
        }
    }

    (format_tags, stream_tags, stream_summaries)
}

fn parse_exiftool_tags(path: &str, tools: &MetadataToolAvailability) -> Vec<MetadataEntry> {
    if !tools.exiftool {
        return Vec::new();
    }

    let exiftool_cmd = find_command("exiftool").unwrap_or_else(|| "exiftool".to_string());
    let output = Command::new(&exiftool_cmd)
        .args(["-json", "-sort", "-fast"])
        .arg(path)
        .output();

    let output = match output {
        Ok(out) if out.status.success() => out,
        Ok(out) => {
            warn!("exiftool failed: {}", String::from_utf8_lossy(&out.stderr));
            return Vec::new();
        }
        Err(err) => {
            warn!("Failed to run exiftool: {}", err);
            return Vec::new();
        }
    };

    let parsed: Value = match serde_json::from_slice(&output.stdout) {
        Ok(val) => val,
        Err(err) => {
            warn!("Failed to parse exiftool output: {}", err);
            return Vec::new();
        }
    };

    let mut entries = Vec::new();
    let Some(array) = parsed.as_array() else {
        return entries;
    };

    if let Some(first) = array.first().and_then(|v| v.as_object()) {
        for (key, value) in first.iter() {
            if key == "SourceFile" {
                continue;
            }

            let val_str = match value {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                Value::Null => continue,
                other => serde_json::to_string(other).unwrap_or_else(|_| "".to_string()),
            };

            entries.push(MetadataEntry {
                key: key.clone(),
                value: val_str,
                scope: MetadataScope::File,
                stream_index: None,
                origin: MetadataOrigin::Exiftool,
                editable: tools.exiftool,
            });
        }
    }

    entries
}

fn create_temp_path(original: &Path, label: &str) -> PathBuf {
    let parent = original.parent().unwrap_or_else(|| Path::new("."));
    let stem = original
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".to_string());
    let ext = original
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_else(String::new);
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);

    parent.join(format!("{stem}.seer_{label}_{ts}{ext}"))
}

fn create_backup_path(original: &Path) -> PathBuf {
    create_temp_path(original, "backup")
}

fn apply_ffmpeg_operations(
    input: &Path,
    output: &Path,
    operations: &[MetadataOperation],
) -> Result<(), String> {
    let ffmpeg_cmd = find_command("ffmpeg").unwrap_or_else(|| "ffmpeg".to_string());
    let mut cmd = Command::new(&ffmpeg_cmd);

    cmd.arg("-y")
        .arg("-i")
        .arg(input)
        .arg("-map")
        .arg("0")
        .arg("-c")
        .arg("copy");

    let mut format_wipe = false;
    let mut stream_wipes: Vec<i32> = Vec::new();

    for op in operations {
        let key = op.key.trim();
        let value = op.value.clone().unwrap_or_default();

        match op.scope {
            MetadataScope::Format => {
                if op.action == MetadataAction::Delete && key == "*" {
                    format_wipe = true;
                } else {
                    cmd.arg("-metadata").arg(format!(
                        "{}={}",
                        key,
                        if op.action == MetadataAction::Delete {
                            String::new()
                        } else {
                            value
                        }
                    ));
                }
            }
            MetadataScope::Stream => {
                let index = op.stream_index.unwrap_or(-1);
                if op.action == MetadataAction::Delete && key == "*" {
                    stream_wipes.push(index);
                } else {
                    cmd.arg(format!("-metadata:s:{}", index)).arg(format!(
                        "{}={}",
                        key,
                        if op.action == MetadataAction::Delete {
                            String::new()
                        } else {
                            value
                        }
                    ));
                }
            }
            MetadataScope::File => {}
        }
    }

    if format_wipe {
        cmd.arg("-map_metadata").arg("-1");
    }

    for idx in stream_wipes {
        cmd.arg(format!("-map_metadata:s:{}", idx)).arg("-1");
    }

    cmd.arg(output);

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run ffmpeg: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "ffmpeg failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(())
}

fn apply_exiftool_operations(
    input: &Path,
    output: &Path,
    operations: &[MetadataOperation],
) -> Result<(), String> {
    let exiftool_cmd = find_command("exiftool").unwrap_or_else(|| "exiftool".to_string());

    let mut cmd = Command::new(&exiftool_cmd);
    cmd.arg("-quiet").arg("-out").arg(output);

    for op in operations {
        let key = op.key.trim();
        let value = op.value.clone().unwrap_or_default();

        match op.action {
            MetadataAction::Set => {
                cmd.arg(format!("-{}={}", key, value));
            }
            MetadataAction::Delete => {
                if key == "*" {
                    cmd.arg("-all=");
                } else {
                    cmd.arg(format!("-{}=", key));
                }
            }
        }
    }

    cmd.arg(input);

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run exiftool: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "exiftool failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(())
}

fn finalize_replacement(original: &Path, new_file: &Path) -> Result<(), String> {
    if original == new_file {
        return Ok(());
    }

    let backup_path = create_backup_path(original);

    fs::rename(original, &backup_path).map_err(|e| format!("Failed to create backup: {}", e))?;

    if let Err(err) = fs::rename(new_file, original) {
        let _ = fs::rename(&backup_path, original);
        return Err(format!("Failed to replace original file: {}", err));
    }

    let _ = fs::remove_file(&backup_path);
    Ok(())
}

pub fn list_metadata(path: String) -> Result<MetadataSnapshot, String> {
    debug!("list_metadata for {}", path);

    let validated = config::validate_path(Path::new(&path))?;
    let tools = detect_tools();

    let file_metadata: FileMetadata = files::get_file_metadata(path.clone())?;

    // Determine file type based on extension
    let extension = file_metadata.extension.as_deref().unwrap_or("");
    let is_image = is_image_extension(extension);
    let is_video_audio = is_video_audio_extension(extension);

    // For images: skip ffprobe data (not relevant)
    // For video/audio: skip exiftool (not relevant)
    let (format_tags, stream_tags, stream_summaries) = if is_image {
        // Images don't need ffprobe data
        (Vec::new(), Vec::new(), Vec::new())
    } else {
        parse_ffprobe_tags(file_metadata.ffprobe_data.clone(), &tools)
    };

    let file_tags = if is_video_audio {
        // Video/audio files don't need EXIF data
        Vec::new()
    } else {
        parse_exiftool_tags(&path, &tools)
    };

    Ok(MetadataSnapshot {
        path: path.clone(),
        file_name: validated
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.clone()),
        size: file_metadata.size,
        modified: file_metadata.modified,
        created: file_metadata.created,
        extension: file_metadata.extension,
        format_tags,
        stream_tags,
        file_tags,
        stream_summaries,
        tool_availability: tools,
    })
}

pub fn update_metadata(
    path: String,
    operations: Vec<MetadataOperation>,
) -> Result<MetadataUpdateResult, String> {
    if operations.is_empty() {
        return Err("No operations provided".to_string());
    }

    let validated = config::validate_path(Path::new(&path))?;
    if !validated.exists() {
        return Err("File does not exist".to_string());
    }

    let tools = detect_tools();

    let ffmpeg_ops: Vec<MetadataOperation> = operations
        .iter()
        .cloned()
        .filter(|op| matches!(op.scope, MetadataScope::Format | MetadataScope::Stream))
        .collect();
    let exif_ops: Vec<MetadataOperation> = operations
        .iter()
        .cloned()
        .filter(|op| matches!(op.scope, MetadataScope::File))
        .collect();

    if !ffmpeg_ops.is_empty() && !tools.ffmpeg {
        return Err("ffmpeg is required to edit container/stream metadata".to_string());
    }

    if !exif_ops.is_empty() && !tools.exiftool {
        return Err("exiftool is required to edit file-level metadata".to_string());
    }

    let mut temp_files: Vec<PathBuf> = Vec::new();
    let mut current_input = validated.clone();

    if !ffmpeg_ops.is_empty() {
        let temp_output = create_temp_path(&validated, "ffmpeg");
        apply_ffmpeg_operations(&current_input, &temp_output, &ffmpeg_ops)?;
        temp_files.push(temp_output.clone());
        current_input = temp_output;
    }

    if !exif_ops.is_empty() {
        let temp_output = create_temp_path(&validated, "exif");
        apply_exiftool_operations(&current_input, &temp_output, &exif_ops)?;
        temp_files.push(temp_output.clone());
        current_input = temp_output;
    }

    if current_input != validated {
        finalize_replacement(&validated, &current_input)?;
    }

    for temp in temp_files {
        if temp != validated {
            let _ = fs::remove_file(temp);
        }
    }

    invalidate_probe_cache(&path);

    Ok(MetadataUpdateResult {
        success: true,
        applied: ffmpeg_ops.into_iter().chain(exif_ops.into_iter()).collect(),
        errors: Vec::new(),
    })
}

pub fn tool_status() -> MetadataToolAvailability {
    detect_tools()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_ffmpeg_delete_all_flag() {
        let ops = vec![MetadataOperation {
            action: MetadataAction::Delete,
            key: "*".to_string(),
            value: None,
            scope: MetadataScope::Format,
            stream_index: None,
        }];

        let temp_in = Path::new("/tmp/input.mp4");
        let temp_out = Path::new("/tmp/output.mp4");
        let args = {
            let ffmpeg_cmd = find_command("ffmpeg").unwrap_or_else(|| "ffmpeg".to_string());
            let mut cmd = Command::new(&ffmpeg_cmd);
            cmd.arg("-y")
                .arg("-i")
                .arg(temp_in)
                .arg("-map")
                .arg("0")
                .arg("-c")
                .arg("copy");
            let mut format_wipe = false;
            for op in &ops {
                if op.action == MetadataAction::Delete
                    && op.key == "*"
                    && matches!(op.scope, MetadataScope::Format)
                {
                    format_wipe = true;
                }
            }
            if format_wipe {
                cmd.arg("-map_metadata").arg("-1");
            }
            cmd.arg(temp_out);
            cmd.get_args()
                .map(|a| a.to_string_lossy().to_string())
                .collect::<Vec<String>>()
        };

        assert!(args.contains(&"-map_metadata".to_string()));
        assert!(args.contains(&"-1".to_string()));
    }
}
