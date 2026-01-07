//! Bulk rename pattern implementations
//!
//! Provides different rename strategies: find/replace, sequential, case transform, template

use crate::media::find_command;
use crate::types::{CaseMode, RenamePattern, RenamePreview};
use log::{debug, warn};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Command;

/// Apply find & replace to a filename
pub fn apply_find_replace(name: &str, find: &str, replace: &str, case_sensitive: bool) -> String {
    if find.is_empty() {
        return name.to_string();
    }

    if case_sensitive {
        name.replace(find, replace)
    } else {
        // Case-insensitive replace
        let lower_name = name.to_lowercase();
        let lower_find = find.to_lowercase();
        let mut result = String::new();
        let mut last_end = 0;

        for (start, _) in lower_name.match_indices(&lower_find) {
            result.push_str(&name[last_end..start]);
            result.push_str(replace);
            last_end = start + find.len();
        }
        result.push_str(&name[last_end..]);
        result
    }
}

/// Apply sequential numbering to multiple files
/// Pattern supports: {n} (number), {name} (filename without ext), {ext} (extension)
/// Examples: "file_{n}" -> "file_001", "{name}_{n}" -> "movie_001", "{n}_{name}.{ext}" -> "001_movie.mp4"
pub fn apply_sequential_numbering(
    paths: &[String],
    pattern: &str,
    start: usize,
    padding: usize,
) -> HashMap<String, String> {
    let mut result = HashMap::new();

    for (i, path) in paths.iter().enumerate() {
        let path_obj = Path::new(path);
        let original_name = path_obj.file_name().and_then(|n| n.to_str()).unwrap_or("");

        // Get extension if exists
        let ext = path_obj.extension().and_then(|e| e.to_str()).unwrap_or("");

        // Get filename stem (without extension)
        let stem = path_obj.file_stem().and_then(|s| s.to_str()).unwrap_or("");

        // Calculate number
        let number = start + i;
        let number_str = format!("{:0width$}", number, width = padding);

        // Replace all placeholders
        let has_placeholders =
            pattern.contains("{n}") || pattern.contains("{name}") || pattern.contains("{ext}");

        let new_name = if has_placeholders {
            pattern
                .replace("{n}", &number_str)
                .replace("{name}", stem)
                .replace("{ext}", ext)
        } else {
            // If no placeholder, append number before extension
            if ext.is_empty() {
                format!("{}_{}", original_name, number_str)
            } else {
                format!("{}_{}.{}", stem, number_str, ext)
            }
        };

        result.insert(path.clone(), new_name);
    }

    result
}

/// Apply case transformation to a filename
pub fn apply_case_transform(name: &str, mode: &CaseMode) -> String {
    match mode {
        CaseMode::Lowercase => name.to_lowercase(),
        CaseMode::Uppercase => name.to_uppercase(),
        CaseMode::TitleCase => {
            // Title case: capitalize first letter of each word
            name.split_whitespace()
                .map(|word| {
                    let mut chars = word.chars();
                    match chars.next() {
                        None => String::new(),
                        Some(first) => {
                            first.to_uppercase().collect::<String>()
                                + &chars.as_str().to_lowercase()
                        }
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        }
    }
}

/// Get media metadata for template variables (type, codecs)
fn get_media_metadata(path: &str) -> Option<(String, Option<String>, Option<String>)> {
    let ffprobe_cmd = find_command("ffprobe")?;

    let output = Command::new(&ffprobe_cmd)
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_streams",
            "-show_format",
            path,
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let json_str = String::from_utf8_lossy(&output.stdout);
    let data: serde_json::Value = serde_json::from_str(&json_str).ok()?;

    let streams = data.get("streams")?.as_array()?;

    // Detect file type and codecs
    let mut file_type = "file".to_string();
    let mut video_codec: Option<String> = None;
    let mut audio_codec: Option<String> = None;

    for stream in streams {
        let codec_type = stream.get("codec_type")?.as_str()?;
        let codec_name = stream.get("codec_name").and_then(|v| v.as_str());

        match codec_type {
            "video" => {
                file_type = "video".to_string();
                if video_codec.is_none() {
                    video_codec = codec_name.map(|s| s.to_string());
                }
            }
            "audio" => {
                if file_type == "file" {
                    file_type = "audio".to_string();
                }
                if audio_codec.is_none() {
                    audio_codec = codec_name.map(|s| s.to_string());
                }
            }
            _ => {}
        }
    }

    Some((file_type, video_codec, audio_codec))
}

/// Apply template pattern with variables
/// Vars: {name}, {ext}, {date}, {index}, {counter}, {parent}, {type}, {video_codec}, {audio_codec}
pub fn apply_template(
    path: &str,
    template: &str,
    index: usize,
    counter: usize,
) -> Result<String, String> {
    let path_obj = Path::new(path);

    let original_name = path_obj.file_stem().and_then(|n| n.to_str()).unwrap_or("");

    let ext = path_obj.extension().and_then(|e| e.to_str()).unwrap_or("");

    let parent = path_obj
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("");

    // Get current date
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();

    let mut result = template.to_string();

    // Replace basic variables
    result = result.replace("{name}", original_name);
    result = result.replace("{ext}", ext);
    result = result.replace("{date}", &date);
    result = result.replace("{index}", &index.to_string());
    result = result.replace("{counter}", &counter.to_string());
    result = result.replace("{parent}", parent);

    // Replace media variables if needed
    if result.contains("{type}")
        || result.contains("{video_codec}")
        || result.contains("{audio_codec}")
    {
        if let Some((file_type, video_codec, audio_codec)) = get_media_metadata(path) {
            result = result.replace("{type}", &file_type);
            result = result.replace("{video_codec}", video_codec.as_deref().unwrap_or("unknown"));
            result = result.replace("{audio_codec}", audio_codec.as_deref().unwrap_or("unknown"));
        } else {
            // Not a media file or ffprobe failed
            result = result.replace("{type}", "file");
            result = result.replace("{video_codec}", "none");
            result = result.replace("{audio_codec}", "none");
        }
    }

    // Ensure we have an extension if original had one
    if !ext.is_empty() && !result.contains('.') {
        result = format!("{}.{}", result, ext);
    }

    Ok(result)
}

/// Generate conflict-free name by appending (1), (2), etc.
fn resolve_conflict(base_name: &str, existing: &HashSet<String>) -> String {
    if !existing.contains(base_name) {
        return base_name.to_string();
    }

    // Split into name and extension
    let path = Path::new(base_name);
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(base_name);

    // Try appending (1), (2), etc.
    for i in 1..10000 {
        let new_name = if ext.is_empty() {
            format!("{} ({})", stem, i)
        } else {
            format!("{} ({}).{}", stem, i, ext)
        };

        if !existing.contains(&new_name) {
            return new_name;
        }
    }

    // Fallback: append timestamp
    let timestamp = chrono::Local::now().timestamp();
    if ext.is_empty() {
        format!("{}_{}", stem, timestamp)
    } else {
        format!("{}_{}.{}", stem, timestamp, ext)
    }
}

/// Preview renames for a set of paths with given pattern
pub fn preview_renames(
    paths: Vec<String>,
    pattern: RenamePattern,
    auto_rename_conflicts: bool,
) -> Result<Vec<RenamePreview>, String> {
    let mut previews = Vec::new();
    let mut seen_names: HashSet<String> = HashSet::new();

    match pattern {
        RenamePattern::FindReplace {
            find,
            replace,
            case_sensitive,
        } => {
            for path in paths {
                let path_obj = Path::new(&path);
                let original_name = path_obj
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();

                let mut new_name =
                    apply_find_replace(&original_name, &find, &replace, case_sensitive);

                // Check for conflicts
                let mut conflict = seen_names.contains(&new_name);
                let error = if new_name.is_empty() {
                    Some("Name cannot be empty".to_string())
                } else {
                    None
                };

                // Auto-rename if enabled and conflict exists
                if conflict && auto_rename_conflicts && error.is_none() {
                    new_name = resolve_conflict(&new_name, &seen_names);
                    conflict = false;
                }

                seen_names.insert(new_name.clone());

                let parent = path_obj.parent().unwrap_or(Path::new(""));
                let new_path = parent.join(&new_name).to_string_lossy().to_string();

                previews.push(RenamePreview {
                    original_path: path.clone(),
                    original_name,
                    new_name,
                    new_path,
                    conflict,
                    error,
                });
            }
        }

        RenamePattern::Sequential {
            pattern: seq_pattern,
            start,
            padding,
        } => {
            let renames = apply_sequential_numbering(&paths, &seq_pattern, start, padding);

            for path in paths {
                let path_obj = Path::new(&path);
                let original_name = path_obj
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();

                let mut new_name = renames.get(&path).cloned().unwrap_or(original_name.clone());

                // Check for conflicts
                let mut conflict = seen_names.contains(&new_name);
                let error = if new_name.is_empty() {
                    Some("Name cannot be empty".to_string())
                } else {
                    None
                };

                // Auto-rename if enabled and conflict exists
                if conflict && auto_rename_conflicts && error.is_none() {
                    new_name = resolve_conflict(&new_name, &seen_names);
                    conflict = false;
                }

                seen_names.insert(new_name.clone());

                let parent = path_obj.parent().unwrap_or(Path::new(""));
                let new_path = parent.join(&new_name).to_string_lossy().to_string();

                previews.push(RenamePreview {
                    original_path: path.clone(),
                    original_name,
                    new_name,
                    new_path,
                    conflict,
                    error,
                });
            }
        }

        RenamePattern::CaseTransform { mode } => {
            for path in paths {
                let path_obj = Path::new(&path);
                let original_name = path_obj
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();

                let mut new_name = apply_case_transform(&original_name, &mode);

                // Check for conflicts
                let mut conflict = seen_names.contains(&new_name);
                let error = if new_name.is_empty() {
                    Some("Name cannot be empty".to_string())
                } else {
                    None
                };

                // Auto-rename if enabled and conflict exists
                if conflict && auto_rename_conflicts && error.is_none() {
                    new_name = resolve_conflict(&new_name, &seen_names);
                    conflict = false;
                }

                seen_names.insert(new_name.clone());

                let parent = path_obj.parent().unwrap_or(Path::new(""));
                let new_path = parent.join(&new_name).to_string_lossy().to_string();

                previews.push(RenamePreview {
                    original_path: path.clone(),
                    original_name,
                    new_name,
                    new_path,
                    conflict,
                    error,
                });
            }
        }

        RenamePattern::Template { template } => {
            for (counter, path) in paths.iter().enumerate() {
                let path_obj = Path::new(&path);
                let original_name = path_obj
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();

                let mut new_name = match apply_template(&path, &template, counter, counter + 1) {
                    Ok(name) => name,
                    Err(e) => {
                        warn!("Template error for {}: {}", path, e);
                        previews.push(RenamePreview {
                            original_path: path.clone(),
                            original_name: original_name.clone(),
                            new_name: original_name,
                            new_path: path.clone(),
                            conflict: false,
                            error: Some(e),
                        });
                        continue;
                    }
                };

                // Check for conflicts
                let mut conflict = seen_names.contains(&new_name);
                let error = if new_name.is_empty() {
                    Some("Name cannot be empty".to_string())
                } else {
                    None
                };

                // Auto-rename if enabled and conflict exists
                if conflict && auto_rename_conflicts && error.is_none() {
                    new_name = resolve_conflict(&new_name, &seen_names);
                    conflict = false;
                }

                seen_names.insert(new_name.clone());

                let parent = path_obj.parent().unwrap_or(Path::new(""));
                let new_path = parent.join(&new_name).to_string_lossy().to_string();

                previews.push(RenamePreview {
                    original_path: path.clone(),
                    original_name,
                    new_name,
                    new_path,
                    conflict,
                    error,
                });
            }
        }
    }

    debug!(
        "Generated {} rename previews, {} conflicts",
        previews.len(),
        previews.iter().filter(|p| p.conflict).count()
    );

    Ok(previews)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========== apply_find_replace tests ==========

    #[test]
    fn test_find_replace_empty_find() {
        let result = apply_find_replace("test.mp4", "", "new", true);
        assert_eq!(result, "test.mp4");
    }

    #[test]
    fn test_find_replace_case_sensitive() {
        let result = apply_find_replace("TestFile.mp4", "Test", "Demo", true);
        assert_eq!(result, "DemoFile.mp4");
    }

    #[test]
    fn test_find_replace_case_sensitive_no_match() {
        let result = apply_find_replace("testfile.mp4", "Test", "Demo", true);
        assert_eq!(result, "testfile.mp4");
    }

    #[test]
    fn test_find_replace_case_insensitive() {
        let result = apply_find_replace("TestFile.mp4", "test", "demo", false);
        assert_eq!(result, "demoFile.mp4");
    }

    #[test]
    fn test_find_replace_case_insensitive_multiple() {
        let result = apply_find_replace("Test Test Test", "test", "demo", false);
        assert_eq!(result, "demo demo demo");
    }

    #[test]
    fn test_find_replace_multiple_occurrences() {
        let result = apply_find_replace("file_old_old.mp4", "old", "new", true);
        assert_eq!(result, "file_new_new.mp4");
    }

    #[test]
    fn test_find_replace_special_chars() {
        let result = apply_find_replace("file-name.mp4", "-", "_", true);
        assert_eq!(result, "file_name.mp4");
    }

    #[test]
    fn test_find_replace_preserve_extension() {
        let result = apply_find_replace("video.mp4", "video", "movie", true);
        assert_eq!(result, "movie.mp4");
    }

    // ========== apply_sequential_numbering tests ==========

    #[test]
    fn test_sequential_simple_pattern() {
        let paths = vec![
            "/path/file1.mp4".to_string(),
            "/path/file2.mp4".to_string(),
            "/path/file3.mp4".to_string(),
        ];
        let result = apply_sequential_numbering(&paths, "video_{n}.{ext}", 1, 3);
        assert_eq!(result[&paths[0]], "video_001.mp4");
        assert_eq!(result[&paths[1]], "video_002.mp4");
        assert_eq!(result[&paths[2]], "video_003.mp4");
    }

    #[test]
    fn test_sequential_with_name_placeholder() {
        let paths = vec!["/path/movie.mp4".to_string(), "/path/show.mp4".to_string()];
        let result = apply_sequential_numbering(&paths, "{name}_{n}.{ext}", 1, 2);
        assert_eq!(result[&paths[0]], "movie_01.mp4");
        assert_eq!(result[&paths[1]], "show_02.mp4");
    }

    #[test]
    fn test_sequential_no_placeholder() {
        let paths = vec!["/path/file.mp4".to_string()];
        let result = apply_sequential_numbering(&paths, "newname", 1, 3);
        assert_eq!(result[&paths[0]], "file_001.mp4");
    }

    #[test]
    fn test_sequential_start_value() {
        let paths = vec!["/path/file.mp4".to_string(), "/path/file2.mp4".to_string()];
        let result = apply_sequential_numbering(&paths, "{n}.{ext}", 10, 2);
        assert_eq!(result[&paths[0]], "10.mp4");
        assert_eq!(result[&paths[1]], "11.mp4");
    }

    #[test]
    fn test_sequential_padding() {
        let paths = vec!["/path/file.mp4".to_string(), "/path/file2.mp4".to_string()];
        let result = apply_sequential_numbering(&paths, "{n}.{ext}", 1, 5);
        assert_eq!(result[&paths[0]], "00001.mp4");
        assert_eq!(result[&paths[1]], "00002.mp4");
    }

    #[test]
    fn test_sequential_no_extension() {
        let paths = vec!["/path/file".to_string()];
        let result = apply_sequential_numbering(&paths, "{n}", 1, 2);
        assert_eq!(result[&paths[0]], "01");
    }

    #[test]
    fn test_sequential_all_placeholders() {
        let paths = vec!["/path/video.mp4".to_string()];
        let result = apply_sequential_numbering(&paths, "{n}_{name}.{ext}", 5, 3);
        assert_eq!(result[&paths[0]], "005_video.mp4");
    }

    // ========== apply_case_transform tests ==========

    #[test]
    fn test_case_lowercase() {
        let result = apply_case_transform("TestFile.MP4", &CaseMode::Lowercase);
        assert_eq!(result, "testfile.mp4");
    }

    #[test]
    fn test_case_uppercase() {
        let result = apply_case_transform("testfile.mp4", &CaseMode::Uppercase);
        assert_eq!(result, "TESTFILE.MP4");
    }

    #[test]
    fn test_case_titlecase() {
        let result = apply_case_transform("the quick brown fox", &CaseMode::TitleCase);
        assert_eq!(result, "The Quick Brown Fox");
    }

    #[test]
    fn test_case_titlecase_mixed() {
        let result = apply_case_transform("tHe QuIcK bRoWn", &CaseMode::TitleCase);
        assert_eq!(result, "The Quick Brown");
    }

    #[test]
    fn test_case_titlecase_single_word() {
        let result = apply_case_transform("hello", &CaseMode::TitleCase);
        assert_eq!(result, "Hello");
    }

    #[test]
    fn test_case_titlecase_empty() {
        let result = apply_case_transform("", &CaseMode::TitleCase);
        assert_eq!(result, "");
    }

    // ========== resolve_conflict tests ==========

    #[test]
    fn test_resolve_conflict_no_conflict() {
        let existing = HashSet::new();
        let result = resolve_conflict("file.mp4", &existing);
        assert_eq!(result, "file.mp4");
    }

    #[test]
    fn test_resolve_conflict_with_conflict() {
        let mut existing = HashSet::new();
        existing.insert("file.mp4".to_string());
        let result = resolve_conflict("file.mp4", &existing);
        assert_eq!(result, "file (1).mp4");
    }

    #[test]
    fn test_resolve_conflict_multiple() {
        let mut existing = HashSet::new();
        existing.insert("file.mp4".to_string());
        existing.insert("file (1).mp4".to_string());
        existing.insert("file (2).mp4".to_string());
        let result = resolve_conflict("file.mp4", &existing);
        assert_eq!(result, "file (3).mp4");
    }

    #[test]
    fn test_resolve_conflict_no_extension() {
        let mut existing = HashSet::new();
        existing.insert("file".to_string());
        let result = resolve_conflict("file", &existing);
        assert_eq!(result, "file (1)");
    }

    #[test]
    fn test_resolve_conflict_preserves_extension() {
        let mut existing = HashSet::new();
        existing.insert("document.txt".to_string());
        let result = resolve_conflict("document.txt", &existing);
        assert_eq!(result, "document (1).txt");
    }

    // ========== apply_template tests ==========

    #[test]
    fn test_template_basic_name() {
        let result = apply_template("/path/file.mp4", "{name}_new", 0, 1).unwrap();
        assert_eq!(result, "file_new.mp4");
    }

    #[test]
    fn test_template_with_index() {
        let result = apply_template("/path/file.mp4", "{name}_{index}", 5, 1).unwrap();
        assert_eq!(result, "file_5.mp4");
    }

    #[test]
    fn test_template_with_counter() {
        let result = apply_template("/path/file.mp4", "{name}_{counter}", 0, 42).unwrap();
        assert_eq!(result, "file_42.mp4");
    }

    #[test]
    fn test_template_with_ext() {
        let result = apply_template("/path/file.mp4", "{name}_backup.{ext}", 0, 1).unwrap();
        assert_eq!(result, "file_backup.mp4");
    }

    #[test]
    fn test_template_with_parent() {
        let result = apply_template("/videos/movies/file.mp4", "{parent}_{name}", 0, 1).unwrap();
        assert_eq!(result, "movies_file.mp4");
    }

    #[test]
    fn test_template_with_date() {
        let result = apply_template("/path/file.mp4", "{name}_{date}", 0, 1).unwrap();
        assert!(result.starts_with("file_"));
        assert!(result.contains("-")); // Date format contains dashes
    }

    #[test]
    fn test_template_no_extension() {
        let result = apply_template("/path/file", "{name}_new", 0, 1).unwrap();
        assert_eq!(result, "file_new");
    }

    #[test]
    fn test_template_auto_add_extension() {
        let result = apply_template("/path/file.mp4", "{name}_copy", 0, 1).unwrap();
        assert_eq!(result, "file_copy.mp4");
    }

    #[test]
    fn test_template_all_vars() {
        let result = apply_template("/videos/file.mp4", "{index}_{counter}_{name}", 3, 10).unwrap();
        assert_eq!(result, "3_10_file.mp4");
    }
}
