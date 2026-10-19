// src/ipc/converter.rs
//! File conversion and file manager IPC utilities for radial-dial.
//! Supports Dolphin, Nautilus, Thunar, etc.

use std::path::{Path, PathBuf};
use std::process::Command;
use crate::state::actions::{ActionId, SliceItem};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileCategory {
    Document,
    Image,
    Pdf,
    Media,
    Other,
}

/// Percent-decodes a URL-encoded string (e.g., "%20" -> " ")
pub fn percent_decode(input: &str) -> String {
    let mut bytes = Vec::with_capacity(input.len());
    let mut chars = input.bytes().peekable();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let h1 = chars.next().and_then(|c| (c as char).to_digit(16));
            let h2 = chars.next().and_then(|c| (c as char).to_digit(16));
            if let (Some(h1), Some(h2)) = (h1, h2) {
                bytes.push(((h1 << 4) | h2) as u8);
                continue;
            }
        }
        bytes.push(b);
    }
    String::from_utf8_lossy(&bytes).to_string()
}

/// Parse a raw line from text/uri-list into a local file path
pub fn parse_uri_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    let path_str = if let Some(stripped) = trimmed.strip_prefix("file://") {
        percent_decode(stripped)
    } else if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        return None;
    };

    Some(path_str)
}

/// Parse multiline text/uri-list or plain text clipboard into list of existing file paths
pub fn parse_clipboard_uris(content: &str) -> Vec<String> {
    let mut files = Vec::new();
    for line in content.lines() {
        if let Some(path) = parse_uri_line(line) {
            let p = Path::new(&path);
            if p.exists() {
                files.push(path);
            }
        }
    }
    files
}

/// Synchronously query Dolphin (via D-Bus edit_copy) and read clipboard
pub fn get_selected_files_sync(pid: i64, _win_title: &str) -> Vec<String> {
    // 1. If Dolphin PID is known, trigger Dolphin's edit_copy action via D-Bus
    if pid > 0 {
        let dest = format!("org.kde.dolphin-{}", pid);
        let _ = Command::new("gdbus")
            .args([
                "call",
                "--session",
                "--dest",
                &dest,
                "--object-path",
                "/dolphin/Dolphin_1",
                "--method",
                "org.kde.KMainWindow.activateAction",
                "edit_copy",
            ])
            .output();
    }

    // 2. Read clipboard as text/uri-list
    if let Ok(output) = Command::new("wl-paste")
        .args(["-t", "text/uri-list"])
        .output()
    {
        if output.status.success() {
            let content = String::from_utf8_lossy(&output.stdout);
            let files = parse_clipboard_uris(&content);
            if !files.is_empty() {
                return files;
            }
        }
    }

    // 3. Fallback: check plain text clipboard for file paths
    if let Ok(output) = Command::new("wl-paste").output() {
        if output.status.success() {
            let content = String::from_utf8_lossy(&output.stdout);
            let files = parse_clipboard_uris(&content);
            if !files.is_empty() {
                return files;
            }
        }
    }

    Vec::new()
}

/// Resolve the current working directory of the file manager window
pub fn resolve_filemanager_dir(pid: i64, win_title: &str, selected: &[String]) -> PathBuf {
    // 1. If files are selected, use parent directory of first file
    if let Some(first) = selected.first() {
        if let Some(parent) = Path::new(first).parent() {
            if parent.is_dir() {
                return parent.to_path_buf();
            }
        }
    }

    // 2. Query Dolphin's copy_location action via D-Bus
    if pid > 0 {
        let dest = format!("org.kde.dolphin-{}", pid);
        let _ = Command::new("gdbus")
            .args([
                "call",
                "--session",
                "--dest",
                &dest,
                "--object-path",
                "/dolphin/Dolphin_1",
                "--method",
                "org.kde.KMainWindow.activateAction",
                "copy_location",
            ])
            .output();

        if let Ok(output) = Command::new("wl-paste").output() {
            if output.status.success() {
                let content = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let path = if let Some(s) = content.strip_prefix("file://") {
                    percent_decode(s)
                } else {
                    content
                };
                let p = Path::new(&path);
                if p.is_dir() {
                    return p.to_path_buf();
                } else if p.is_file() {
                    if let Some(parent) = p.parent() {
                        return parent.to_path_buf();
                    }
                }
            }
        }
    }

    // 3. Try parsing directory from window title (e.g. "Downloads — Dolphin")
    let cleaned_title = win_title
        .split(" — ")
        .next()
        .or_else(|| win_title.split(" - ").next())
        .unwrap_or(win_title)
        .trim();

    if !cleaned_title.is_empty() {
        // Direct absolute path
        let direct = Path::new(cleaned_title);
        if direct.is_dir() {
            return direct.to_path_buf();
        }

        // Check relative to home (~/Downloads, ~/Documents, etc.)
        let home = shellexpand::tilde("~").to_string();
        let candidate = Path::new(&home).join(cleaned_title);
        if candidate.is_dir() {
            return candidate;
        }

        // Special common KDE names
        let home_subdirs = ["Downloads", "Documents", "Pictures", "Music", "Videos", "Desktop"];
        for sub in &home_subdirs {
            if cleaned_title.eq_ignore_ascii_case(sub) {
                let p = Path::new(&home).join(sub);
                if p.is_dir() {
                    return p;
                }
            }
        }
    }

    // Default to home directory
    PathBuf::from(shellexpand::tilde("~").to_string())
}

/// Categorize file by its extension
pub fn categorize_file(path: &str) -> FileCategory {
    let p = Path::new(path);
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "pdf" => FileCategory::Pdf,
        "ppt" | "pptx" | "doc" | "docx" | "odt" | "odp" | "rtf" | "txt" | "md" | "xls" | "xlsx" | "csv" | "tsv" | "html" => {
            FileCategory::Document
        }
        "png" | "jpg" | "jpeg" | "webp" | "ico" | "bmp" | "svg" | "gif" | "tiff" | "tif" | "avif" | "heic" | "tga" => {
            FileCategory::Image
        }
        "mp4" | "mkv" | "webm" | "avi" | "mov" | "flv" | "wmv" | "mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" => {
            FileCategory::Media
        }
        _ => FileCategory::Other,
    }
}

/// Generate sub-slices for FileConverter based on selected files
pub fn generate_converter_sub_slices(selected_files: &[String]) -> Vec<SliceItem> {
    if selected_files.is_empty() {
        // Edge case: No file selected
        return vec![
            SliceItem {
                id: "convert_pick_file".into(),
                label: "Pick File...".into(),
                icon: "folder_open".into(),
                has_sub_tier: false,
                sub_tier_type: None,
                action: Some(ActionId::PickAndConvertFile),
                slot_index: 0,
                target_path: None,
                target_index: None,
                is_add_button: false,
            },
            SliceItem {
                id: "convert_select_hint".into(),
                label: "Select in Dolphin".into(),
                icon: "touch_app".into(),
                has_sub_tier: false,
                sub_tier_type: None,
                action: None,
                slot_index: 1,
                target_path: None,
                target_index: None,
                is_add_button: false,
            },
        ];
    }

    if selected_files.len() == 1 {
        // Single file selected
        let file = &selected_files[0];
        let category = categorize_file(file);
        let ext = Path::new(file)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        match category {
            FileCategory::Document => {
                // Single PPT or Word doc -> Convert to PDF or Text
                vec![
                    create_convert_slice(
                        "convert_single_pdf",
                        "Convert to PDF",
                        "picture_as_pdf",
                        0,
                        selected_files,
                        "pdf",
                        false,
                    ),
                    create_convert_slice(
                        "convert_single_txt",
                        "To Text (.txt)",
                        "description",
                        1,
                        selected_files,
                        "txt",
                        false,
                    ),
                ]
            }
            FileCategory::Image => {
                // Single image -> Convert to .ico, PNG, JPG, WEBP, PDF
                let mut slices = Vec::new();
                let mut idx = 0;

                // Always offer .ico
                slices.push(create_convert_slice(
                    "convert_single_ico",
                    "To .ico",
                    "app_shortcut",
                    idx,
                    selected_files,
                    "ico",
                    false,
                ));
                idx += 1;

                if ext != "png" {
                    slices.push(create_convert_slice(
                        "convert_single_png",
                        "To PNG",
                        "photo",
                        idx,
                        selected_files,
                        "png",
                        false,
                    ));
                    idx += 1;
                }
                if ext != "jpg" && ext != "jpeg" {
                    slices.push(create_convert_slice(
                        "convert_single_jpg",
                        "To JPG",
                        "image",
                        idx,
                        selected_files,
                        "jpg",
                        false,
                    ));
                    idx += 1;
                }
                if ext != "webp" {
                    slices.push(create_convert_slice(
                        "convert_single_webp",
                        "To WEBP",
                        "web",
                        idx,
                        selected_files,
                        "webp",
                        false,
                    ));
                    idx += 1;
                }
                slices.push(create_convert_slice(
                    "convert_single_pdf",
                    "To PDF",
                    "picture_as_pdf",
                    idx,
                    selected_files,
                    "pdf",
                    false,
                ));

                slices
            }
            FileCategory::Pdf => {
                vec![
                    create_convert_slice(
                        "convert_pdf_to_png",
                        "To PNG Images",
                        "photo_library",
                        0,
                        selected_files,
                        "png",
                        false,
                    ),
                    create_convert_slice(
                        "convert_pdf_to_jpg",
                        "To JPG Images",
                        "image",
                        1,
                        selected_files,
                        "jpg",
                        false,
                    ),
                    create_convert_slice(
                        "convert_pdf_to_txt",
                        "Extract Text",
                        "description",
                        2,
                        selected_files,
                        "txt",
                        false,
                    ),
                ]
            }
            FileCategory::Media => {
                vec![
                    create_convert_slice(
                        "convert_media_mp3",
                        "Extract MP3",
                        "music_note",
                        0,
                        selected_files,
                        "mp3",
                        false,
                    ),
                    create_convert_slice(
                        "convert_media_gif",
                        "To GIF",
                        "gif",
                        1,
                        selected_files,
                        "gif",
                        false,
                    ),
                    create_convert_slice(
                        "convert_media_mp4",
                        "To MP4",
                        "movie",
                        2,
                        selected_files,
                        "mp4",
                        false,
                    ),
                ]
            }
            FileCategory::Other => {
                vec![
                    create_convert_slice(
                        "convert_other_pdf",
                        "Convert to PDF",
                        "picture_as_pdf",
                        0,
                        selected_files,
                        "pdf",
                        false,
                    ),
                ]
            }
        }
    } else {
        // Multiple files selected!
        // Choice: Either merge them all together, or convert individually
        let mut slices = Vec::new();
        let mut idx = 0;

        let has_docs = selected_files.iter().any(|f| categorize_file(f) == FileCategory::Document);
        let has_images = selected_files.iter().any(|f| categorize_file(f) == FileCategory::Image);
        let has_pdfs = selected_files.iter().any(|f| categorize_file(f) == FileCategory::Pdf);
        let all_images = selected_files.iter().all(|f| categorize_file(f) == FileCategory::Image);

        // --- Option 1: MERGE ALL TOGETHER ---
        slices.push(create_convert_slice(
            "convert_merge_pdf",
            "Merge to PDF",
            "merge",
            idx,
            selected_files,
            "pdf",
            true, // merge = true!
        ));
        idx += 1;

        if all_images {
            slices.push(create_convert_slice(
                "convert_merge_gif",
                "Merge to GIF",
                "gif",
                idx,
                selected_files,
                "gif",
                true, // merge = true!
            ));
            idx += 1;
        }

        // --- Option 2: CONVERT INDIVIDUALLY ---
        if has_docs || has_pdfs {
            slices.push(create_convert_slice(
                "convert_each_pdf",
                "Each to PDF",
                "picture_as_pdf",
                idx,
                selected_files,
                "pdf",
                false, // merge = false!
            ));
            idx += 1;
        }

        if has_images {
            slices.push(create_convert_slice(
                "convert_each_ico",
                "Each to .ico",
                "app_shortcut",
                idx,
                selected_files,
                "ico",
                false,
            ));
            idx += 1;

            slices.push(create_convert_slice(
                "convert_each_png",
                "Each to PNG",
                "photo",
                idx,
                selected_files,
                "png",
                false,
            ));
            idx += 1;

            slices.push(create_convert_slice(
                "convert_each_webp",
                "Each to WEBP",
                "web",
                idx,
                selected_files,
                "webp",
                false,
            ));
            idx += 1;

            slices.push(create_convert_slice(
                "convert_each_jpg",
                "Each to JPG",
                "image",
                idx,
                selected_files,
                "jpg",
                false,
            ));
        }

        slices
    }
}

fn create_convert_slice(
    id: &str,
    label: &str,
    icon: &str,
    slot_index: usize,
    files: &[String],
    target_format: &str,
    merge: bool,
) -> SliceItem {
    SliceItem {
        id: id.to_string(),
        label: label.to_string(),
        icon: icon.to_string(),
        has_sub_tier: false,
        sub_tier_type: None,
        action: Some(ActionId::ConvertFile {
            files: files.to_vec(),
            target_format: target_format.to_string(),
            merge,
        }),
        slot_index,
        target_path: None,
        target_index: None,
        is_add_button: false,
    }
}

/// Helper to generate a unique destination file name avoiding overwrite
pub fn get_unique_output_path(dir: &Path, base_stem: &str, ext: &str) -> PathBuf {
    let mut candidate = dir.join(format!("{}.{}", base_stem, ext));
    if !candidate.exists() {
        return candidate;
    }
    let mut counter = 1;
    loop {
        candidate = dir.join(format!("{}_{}.{}", base_stem, counter, ext));
        if !candidate.exists() {
            return candidate;
        }
        counter += 1;
    }
}

/// Send desktop notification helper
pub fn notify(title: &str, body: &str, is_error: bool) {
    let icon = if is_error { "dialog-error" } else { "dialog-information" };
    let urgency = if is_error { "critical" } else { "normal" };
    let _ = Command::new("notify-send")
        .args([
            "-a",
            "Radial Dial",
            "-i",
            icon,
            "-u",
            urgency,
            title,
            body,
        ])
        .spawn();
}

/// Execute the file conversion or merging in a detached background thread
pub fn execute_conversion(files: &[String], target_format: &str, merge: bool) {
    let files = files.to_vec();
    let target_format = target_format.to_lowercase();

    std::thread::spawn(move || {
        let valid_files: Vec<PathBuf> = files
            .iter()
            .map(PathBuf::from)
            .filter(|p| p.is_file())
            .collect();

        if valid_files.is_empty() {
            notify("File Converter", "No valid files found to convert", true);
            return;
        }

        let out_dir = match valid_files[0].parent() {
            Some(p) if p.is_dir() => p.to_path_buf(),
            _ => PathBuf::from(shellexpand::tilde("~/Downloads").to_string()),
        };

        if merge {
            // MERGE WORKFLOW
            execute_merge(&valid_files, &out_dir, &target_format);
        } else {
            // INDIVIDUAL CONVERSION WORKFLOW
            execute_individual(&valid_files, &out_dir, &target_format);
        }
    });
}

fn execute_merge(files: &[PathBuf], out_dir: &Path, target_format: &str) {
    if target_format == "gif" {
        // Merge images into animated GIF
        let first_stem = files[0].file_stem().and_then(|s| s.to_str()).unwrap_or("merged");
        let out_path = get_unique_output_path(out_dir, &format!("{}_merged", first_stem), "gif");

        let mut cmd = Command::new("magick");
        cmd.args(["-delay", "40", "-loop", "0"]);
        for f in files {
            cmd.arg(f);
        }
        cmd.arg(&out_path);

        match cmd.output() {
            Ok(output) if output.status.success() => {
                let name = out_path.file_name().and_then(|s| s.to_str()).unwrap_or("animation.gif");
                notify("Merge Complete", &format!("Created GIF: {}", name), false);
            }
            _ => {
                notify("Merge Failed", "Could not create animated GIF from images", true);
            }
        }
        return;
    }

    // Default merge target is PDF
    let first_stem = files[0].file_stem().and_then(|s| s.to_str()).unwrap_or("merged");
    let out_path = get_unique_output_path(out_dir, &format!("{}_merged", first_stem), "pdf");

    // Check if all files are already PDF
    let all_pdf = files.iter().all(|f| {
        f.extension().and_then(|e| e.to_str()).unwrap_or("").eq_ignore_ascii_case("pdf")
    });

    if all_pdf {
        let mut cmd = Command::new("pdfunite");
        for f in files {
            cmd.arg(f);
        }
        cmd.arg(&out_path);
        match cmd.output() {
            Ok(output) if output.status.success() => {
                let name = out_path.file_name().and_then(|s| s.to_str()).unwrap_or("merged.pdf");
                notify("Merge Complete", &format!("Merged {} PDFs into {}", files.len(), name), false);
            }
            _ => notify("Merge Failed", "pdfunite failed to merge PDF files", true),
        }
        return;
    }

    // Check if all files are images
    let all_images = files.iter().all(|f| {
        categorize_file(f.to_str().unwrap_or("")) == FileCategory::Image
    });

    if all_images {
        let mut cmd = Command::new("magick");
        for f in files {
            cmd.arg(f);
        }
        cmd.arg(&out_path);
        match cmd.output() {
            Ok(output) if output.status.success() => {
                let name = out_path.file_name().and_then(|s| s.to_str()).unwrap_or("merged.pdf");
                notify("Merge Complete", &format!("Merged {} images into {}", files.len(), name), false);
            }
            _ => notify("Merge Failed", "ImageMagick failed to merge images into PDF", true),
        }
        return;
    }

    // Mixed files (Documents + Images + PDFs)
    // Convert each non-PDF to temporary PDF, then pdfunite
    let temp_dir = std::env::temp_dir().join(format!("radial_merge_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);

    let mut pdf_parts = Vec::new();
    let mut success = true;

    for (i, f) in files.iter().enumerate() {
        let ext = f.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "pdf" {
            pdf_parts.push(f.clone());
        } else if categorize_file(f.to_str().unwrap_or("")) == FileCategory::Document {
            // Convert document via libreoffice
            let status = Command::new("libreoffice")
                .args(["--headless", "--convert-to", "pdf", f.to_str().unwrap_or(""), "--outdir", temp_dir.to_str().unwrap_or("/tmp")])
                .status();
            let stem = f.file_stem().and_then(|s| s.to_str()).unwrap_or("doc");
            let temp_pdf = temp_dir.join(format!("{}.pdf", stem));
            if status.map(|s| s.success()).unwrap_or(false) && temp_pdf.exists() {
                pdf_parts.push(temp_pdf);
            } else {
                success = false;
                break;
            }
        } else if categorize_file(f.to_str().unwrap_or("")) == FileCategory::Image {
            // Convert image via magick
            let temp_pdf = temp_dir.join(format!("img_part_{}.pdf", i));
            let status = Command::new("magick")
                .arg(f)
                .arg(&temp_pdf)
                .status();
            if status.map(|s| s.success()).unwrap_or(false) && temp_pdf.exists() {
                pdf_parts.push(temp_pdf);
            } else {
                success = false;
                break;
            }
        }
    }

    if success && !pdf_parts.is_empty() {
        let mut cmd = Command::new("pdfunite");
        for part in &pdf_parts {
            cmd.arg(part);
        }
        cmd.arg(&out_path);
        match cmd.output() {
            Ok(output) if output.status.success() => {
                let name = out_path.file_name().and_then(|s| s.to_str()).unwrap_or("merged.pdf");
                notify("Merge Complete", &format!("Merged {} files into {}", files.len(), name), false);
            }
            _ => notify("Merge Failed", "pdfunite failed on combined documents", true),
        }
    } else {
        notify("Merge Failed", "Could not convert intermediate documents to PDF", true);
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
}

fn execute_individual(files: &[PathBuf], out_dir: &Path, target_format: &str) {
    let mut converted_count = 0;
    let total = files.len();

    for f in files {
        let category = categorize_file(f.to_str().unwrap_or(""));
        let stem = f.file_stem().and_then(|s| s.to_str()).unwrap_or("file");

        let success = match (category, target_format) {
            // Document to PDF
            (FileCategory::Document, "pdf") => {
                let status = Command::new("libreoffice")
                    .args([
                        "--headless",
                        "--convert-to",
                        "pdf",
                        f.to_str().unwrap_or(""),
                        "--outdir",
                        out_dir.to_str().unwrap_or("."),
                    ])
                    .status();
                status.map(|s| s.success()).unwrap_or(false)
            }
            // Document to Text or other office format
            (FileCategory::Document, fmt) => {
                let status = Command::new("libreoffice")
                    .args([
                        "--headless",
                        "--convert-to",
                        fmt,
                        f.to_str().unwrap_or(""),
                        "--outdir",
                        out_dir.to_str().unwrap_or("."),
                    ])
                    .status();
                status.map(|s| s.success()).unwrap_or(false)
            }
            // Image to .ico
            (FileCategory::Image, "ico") => {
                let out_path = get_unique_output_path(out_dir, stem, "ico");
                let status = Command::new("magick")
                    .arg(f)
                    .args(["-define", "icon:auto-resize=256,128,64,48,32,16"])
                    .arg(&out_path)
                    .status();
                status.map(|s| s.success()).unwrap_or(false)
            }
            // Image to other image format or PDF
            (FileCategory::Image, fmt) => {
                let out_path = get_unique_output_path(out_dir, stem, fmt);
                let status = Command::new("magick")
                    .arg(f)
                    .arg(&out_path)
                    .status();
                status.map(|s| s.success()).unwrap_or(false)
            }
            // PDF to images
            (FileCategory::Pdf, "png") => {
                let prefix = out_dir.join(format!("{}_page", stem));
                let status = Command::new("pdftoppm")
                    .args(["-png", "-r", "150"])
                    .arg(f)
                    .arg(&prefix)
                    .status();
                status.map(|s| s.success()).unwrap_or(false)
            }
            (FileCategory::Pdf, "jpg") | (FileCategory::Pdf, "jpeg") => {
                let prefix = out_dir.join(format!("{}_page", stem));
                let status = Command::new("pdftoppm")
                    .args(["-jpeg", "-r", "150"])
                    .arg(f)
                    .arg(&prefix)
                    .status();
                status.map(|s| s.success()).unwrap_or(false)
            }
            // Media to MP3
            (FileCategory::Media, "mp3") => {
                let out_path = get_unique_output_path(out_dir, stem, "mp3");
                let status = Command::new("ffmpeg")
                    .args(["-y", "-i"])
                    .arg(f)
                    .args(["-vn", "-q:a", "2"])
                    .arg(&out_path)
                    .status();
                status.map(|s| s.success()).unwrap_or(false)
            }
            // Media to GIF
            (FileCategory::Media, "gif") => {
                let out_path = get_unique_output_path(out_dir, stem, "gif");
                let status = Command::new("ffmpeg")
                    .args(["-y", "-i"])
                    .arg(f)
                    .args(["-vf", "fps=10,scale=480:-1:flags=lanczos"])
                    .arg(&out_path)
                    .status();
                status.map(|s| s.success()).unwrap_or(false)
            }
            // Fallback: try ImageMagick
            _ => {
                let out_path = get_unique_output_path(out_dir, stem, target_format);
                let status = Command::new("magick")
                    .arg(f)
                    .arg(&out_path)
                    .status();
                status.map(|s| s.success()).unwrap_or(false)
            }
        };

        if success {
            converted_count += 1;
        }
    }

    if converted_count > 0 {
        notify(
            "Conversion Complete",
            &format!("Successfully converted {}/{} files to {}", converted_count, total, target_format),
            false,
        );
    } else {
        notify(
            "Conversion Failed",
            &format!("Failed to convert files to {}", target_format),
            true,
        );
    }
}

/// Prompt user to pick a file via kdialog, then present conversion options
pub fn pick_and_convert_dialog() {
    std::thread::spawn(|| {
        let home = shellexpand::tilde("~").to_string();
        let output = Command::new("kdialog")
            .args(["--getopenfilename", &home, "All Files (*)"])
            .output();

        if let Ok(out) = output {
            if out.status.success() {
                let file_path = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !file_path.is_empty() && Path::new(&file_path).is_file() {
                    let cat = categorize_file(&file_path);
                    let target_fmt = match cat {
                        FileCategory::Document => "pdf",
                        FileCategory::Image => "ico",
                        FileCategory::Pdf => "png",
                        FileCategory::Media => "mp3",
                        _ => "pdf",
                    };
                    execute_conversion(&[file_path], target_fmt, false);
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_percent_decode() {
        assert_eq!(percent_decode("hello%20world"), "hello world");
        assert_eq!(percent_decode("file%3A%2F%2Ftest"), "file://test");
        assert_eq!(percent_decode("no_encoding"), "no_encoding");
    }

    #[test]
    fn test_parse_uri_line() {
        assert_eq!(
            parse_uri_line("file:///home/astro/Downloads/photo.png"),
            Some("/home/astro/Downloads/photo.png".to_string())
        );
        assert_eq!(
            parse_uri_line("file:///home/astro/My%20Documents/test%20file.pdf"),
            Some("/home/astro/My Documents/test file.pdf".to_string())
        );
        assert_eq!(
            parse_uri_line("/direct/path/file.txt"),
            Some("/direct/path/file.txt".to_string())
        );
        assert_eq!(parse_uri_line("# comment"), None);
        assert_eq!(parse_uri_line(""), None);
    }

    #[test]
    fn test_categorize_file() {
        assert_eq!(categorize_file("slide.pptx"), FileCategory::Document);
        assert_eq!(categorize_file("doc.docx"), FileCategory::Document);
        assert_eq!(categorize_file("readme.txt"), FileCategory::Document);
        assert_eq!(categorize_file("picture.png"), FileCategory::Image);
        assert_eq!(categorize_file("icon.ico"), FileCategory::Image);
        assert_eq!(categorize_file("photo.webp"), FileCategory::Image);
        assert_eq!(categorize_file("manual.pdf"), FileCategory::Pdf);
        assert_eq!(categorize_file("video.mp4"), FileCategory::Media);
        assert_eq!(categorize_file("song.mp3"), FileCategory::Media);
    }

    #[test]
    fn test_generate_converter_sub_slices_empty() {
        let slices = generate_converter_sub_slices(&[]);
        assert!(!slices.is_empty());
        assert_eq!(slices[0].id, "convert_pick_file");
    }

    #[test]
    fn test_generate_converter_sub_slices_single_document() {
        let files = vec!["/home/astro/Downloads/slides.pptx".to_string()];
        let slices = generate_converter_sub_slices(&files);
        assert_eq!(slices.len(), 2);
        assert_eq!(slices[0].label, "Convert to PDF");
        assert_eq!(slices[0].icon, "picture_as_pdf");
        assert_eq!(slices[1].label, "To Text (.txt)");
        assert_eq!(slices[1].icon, "description");
        if let Some(ActionId::ConvertFile { files: f, target_format, merge }) = &slices[0].action {
            assert_eq!(f, &files);
            assert_eq!(target_format, "pdf");
            assert!(!merge);
        } else {
            panic!("Expected ConvertFile action");
        }
    }

    #[test]
    fn test_generate_converter_sub_slices_single_image() {
        let files = vec!["/home/astro/Downloads/photo.png".to_string()];
        let slices = generate_converter_sub_slices(&files);
        // Offers .ico, JPG, WEBP, PDF (skips PNG since source is png)
        assert!(slices.iter().any(|s| s.label == "To .ico"));
        assert!(slices.iter().any(|s| s.label == "To JPG"));
        assert!(slices.iter().any(|s| s.label == "To WEBP"));
        assert!(slices.iter().any(|s| s.label == "To PDF"));
    }

    #[test]
    fn test_generate_converter_sub_slices_multiple_files_merge_and_individual() {
        let files = vec![
            "/home/astro/Downloads/photo1.png".to_string(),
            "/home/astro/Downloads/photo2.jpg".to_string(),
        ];
        let slices = generate_converter_sub_slices(&files);
        // Must contain "Merge to PDF" (merge: true)
        let merge_slice = slices.iter().find(|s| s.label == "Merge to PDF").expect("Merge to PDF slice missing");
        if let Some(ActionId::ConvertFile { merge, target_format, .. }) = &merge_slice.action {
            assert!(merge);
            assert_eq!(target_format, "pdf");
        }

        // Must contain individual conversion options (merge: false)
        let ico_slice = slices.iter().find(|s| s.label == "Each to .ico").expect("Each to .ico slice missing");
        if let Some(ActionId::ConvertFile { merge, target_format, .. }) = &ico_slice.action {
            assert!(!merge);
            assert_eq!(target_format, "ico");
        }
    }

    #[test]
    fn test_get_unique_output_path() {
        let temp_dir = std::env::temp_dir();
        let path = get_unique_output_path(&temp_dir, "radial_test_unique_file", "pdf");
        assert!(path.to_string_lossy().ends_with(".pdf"));
    }
}
