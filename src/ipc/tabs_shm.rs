// src/ipc/tabs_shm.rs

use crate::state::menu::BrowserTab;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub const SHM_TABS_PATH: &str = "/dev/shm/browser_tabs.json";

/// Encode a message payload with a 4-byte little-endian length prefix.
pub fn encode_framed_message(payload: &[u8]) -> Vec<u8> {
    let len = payload.len() as u32;
    let mut buf = Vec::with_capacity(4 + payload.len());
    buf.extend_from_slice(&len.to_le_bytes());
    buf.extend_from_slice(payload);
    buf
}

/// Decode a 4-byte little-endian length prefix into a message length.
pub fn decode_frame_length(bytes: [u8; 4]) -> u32 {
    u32::from_le_bytes(bytes)
}

/// Read a length-prefixed message from a reader.
/// Returns Ok(None) on clean EOF at the start of a frame boundary.
/// Returns Err(UnexpectedEof) on EOF mid-header or mid-payload.
pub fn read_framed_message<R: Read>(reader: &mut R) -> std::io::Result<Option<Vec<u8>>> {
    let mut len_buf = [0u8; 4];
    let mut n = 0;
    while n < 4 {
        match reader.read(&mut len_buf[n..])? {
            0 => {
                if n == 0 {
                    return Ok(None);
                } else {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "Unexpected EOF reading message length header",
                    ));
                }
            }
            bytes_read => n += bytes_read,
        }
    }

    let len = decode_frame_length(len_buf) as usize;
    let mut payload = vec![0u8; len];
    reader.read_exact(&mut payload)?;
    Ok(Some(payload))
}

/// Write a length-prefixed message to a writer and flush.
pub fn write_framed_message<W: Write>(writer: &mut W, payload: &[u8]) -> std::io::Result<()> {
    let len = payload.len() as u32;
    writer.write_all(&len.to_le_bytes())?;
    writer.write_all(payload)?;
    writer.flush()?;
    Ok(())
}

/// Parse browser tabs from a JSON string.
/// Supports both a top-level array `[...]` and an object wrapping tabs `{"tabs": [...]}`.
pub fn read_tabs_from_str(data: &str) -> Option<Vec<BrowserTab>> {
    if let Ok(tabs) = serde_json::from_str::<Vec<BrowserTab>>(data) {
        return Some(tabs);
    }
    #[derive(serde::Deserialize)]
    struct WrappedTabs {
        tabs: Vec<BrowserTab>,
    }
    if let Ok(wrapped) = serde_json::from_str::<WrappedTabs>(data) {
        return Some(wrapped.tabs);
    }
    None
}

/// Read current tabs from a file path if it exists and contains valid JSON.
pub fn read_tabs_from_path<P: AsRef<Path>>(path: P) -> Option<Vec<BrowserTab>> {
    let path = path.as_ref();
    if !path.exists() {
        return None;
    }
    let data = std::fs::read_to_string(path).ok()?;
    read_tabs_from_str(&data)
}

/// Read current tabs from SHM file if it exists.
pub fn read_shm_tabs() -> Option<Vec<BrowserTab>> {
    read_tabs_from_path(SHM_TABS_PATH)
}

/// Atomically write browser tabs to a destination path (writes to `.tmp` then renames).
pub fn write_tabs_atomically<P: AsRef<Path>>(target_path: P, tabs: &[BrowserTab]) -> std::io::Result<()> {
    let target = target_path.as_ref();
    if let Some(parent) = target.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let tmp_path = format!("{}.tmp.{}", target.display(), std::process::id());
    let tmp = Path::new(&tmp_path);
    let json_bytes = serde_json::to_vec(tabs)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(tmp, json_bytes)?;
    std::fs::rename(tmp, target)?;
    Ok(())
}

/// Asynchronously watch a file path for modifications and send updated tabs over a watch channel.
pub async fn watch_tabs_path<P: AsRef<Path>>(
    path: P,
    tx: tokio::sync::watch::Sender<Vec<BrowserTab>>,
) {
    let path = path.as_ref();
    // Read initial tabs
    if let Some(tabs) = read_tabs_from_path(path) {
        let _ = tx.send(tabs);
    }

    let watch_dir = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };

    if !watch_dir.exists() {
        let _ = std::fs::create_dir_all(watch_dir);
    }
    if !watch_dir.exists() {
        log::warn!("Directory does not exist for watching: {}", watch_dir.display());
        return;
    }

    let (notify_tx, mut notify_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut watcher = match RecommendedWatcher::new(
        move |res| {
            let _ = notify_tx.send(res);
        },
        Config::default(),
    ) {
        Ok(w) => w,
        Err(e) => {
            log::error!("Failed to create filesystem watcher for tabs: {e}");
            return;
        }
    };

    if let Err(e) = watcher.watch(watch_dir, RecursiveMode::NonRecursive) {
        log::error!("Failed to watch directory {}: {e}", watch_dir.display());
        return;
    }

    let target_name = path.file_name().map(|n| n.to_os_string());

    while let Some(res) = notify_rx.recv().await {
        match res {
            Ok(event) => {
                let matches = event.paths.iter().any(|p| {
                    if let Some(ref target) = target_name {
                        p.file_name() == Some(target.as_os_str())
                    } else {
                        p == path
                    }
                });

                if matches {
                    if let Some(tabs) = read_tabs_from_path(path) {
                        if tx.send(tabs).is_err() {
                            // Watch channel closed (all receivers dropped)
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                log::warn!("Filesystem watcher error: {e}");
            }
        }
    }
}

/// Asynchronously watch SHM file for modifications and send updated tabs over a watch channel.
pub async fn watch_tabs(tx: tokio::sync::watch::Sender<Vec<BrowserTab>>) {
    watch_tabs_path(Path::new(SHM_TABS_PATH), tx).await;
}

/// Decompress Mozilla's MozLz4 format (used in recovery.jsonlz4, search.json.mozlz4, etc.)
/// Format: 8-byte header `mozLz40\0` + 4-byte uncompressed size (LE) + raw LZ4 block.
pub fn decompress_mozlz4(bytes: &[u8]) -> Result<Vec<u8>, &'static str> {
    const MOZ_MAGIC: &[u8; 8] = b"mozLz40\0";
    if bytes.len() < 12 {
        return Err("Payload too short for mozLz4 header");
    }
    if &bytes[..8] != MOZ_MAGIC {
        return Err("Invalid mozLz4 magic header");
    }

    let uncompressed_size = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
    let mut dest = Vec::with_capacity(uncompressed_size.min(32 * 1024 * 1024));
    let compressed = &bytes[12..];
    let mut pos = 0;

    while pos < compressed.len() {
        let token = compressed[pos];
        pos += 1;
        let mut lit_len = (token >> 4) as usize;
        if lit_len == 15 {
            loop {
                if pos >= compressed.len() {
                    break;
                }
                let s = compressed[pos] as usize;
                pos += 1;
                lit_len += s;
                if s != 255 {
                    break;
                }
            }
        }

        if pos + lit_len > compressed.len() {
            let avail = compressed.len().saturating_sub(pos);
            dest.extend_from_slice(&compressed[pos..pos + avail]);
            break;
        }

        dest.extend_from_slice(&compressed[pos..pos + lit_len]);
        pos += lit_len;

        if pos >= compressed.len() {
            break;
        }

        if pos + 2 > compressed.len() {
            break;
        }
        let offset = u16::from_le_bytes([compressed[pos], compressed[pos + 1]]) as usize;
        pos += 2;
        if offset == 0 {
            return Err("Invalid 0 offset in LZ4 block");
        }

        let mut match_len = ((token & 0x0f) as usize) + 4;
        if (token & 0x0f) == 15 {
            loop {
                if pos >= compressed.len() {
                    break;
                }
                let s = compressed[pos] as usize;
                pos += 1;
                match_len += s;
                if s != 255 {
                    break;
                }
            }
        }

        if dest.len() < offset {
            return Err("LZ4 offset exceeds current decompressed length");
        }

        let start_pos = dest.len() - offset;
        for i in 0..match_len {
            let b = dest[start_pos + (i % offset)];
            dest.push(b);
        }
    }

    Ok(dest)
}

fn get_tab_title_and_url(tab: &serde_json::Value) -> (String, String) {
    if let Some(entries) = tab.get("entries").and_then(|e| e.as_array()) {
        if !entries.is_empty() {
            let ent_idx = tab.get("index").and_then(|i| i.as_u64()).unwrap_or(entries.len() as u64) as usize;
            let target_idx = if ent_idx >= 1 && ent_idx <= entries.len() {
                ent_idx - 1
            } else {
                entries.len() - 1
            };
            let entry = &entries[target_idx];
            let title = entry.get("title").and_then(|t| t.as_str()).unwrap_or("").to_string();
            let url = entry.get("url").and_then(|u| u.as_str()).unwrap_or("").to_string();
            return (title, url);
        }
    }
    ("".to_string(), "".to_string())
}

/// Parse Firefox/Zen session JSON data and extract open tabs for the target window
pub fn parse_firefox_session_tabs(data: &[u8], target_title: Option<&str>) -> Option<Vec<BrowserTab>> {
    let json_val: serde_json::Value = serde_json::from_slice(data).ok()?;
    let windows = json_val.get("windows")?.as_array()?;
    if windows.is_empty() {
        return None;
    }

    let target_clean = target_title.map(|t| t.to_lowercase());
    let mut selected_win: Option<&serde_json::Value> = None;

    if let Some(target) = &target_clean {
        for win in windows {
            if let Some(tabs) = win.get("tabs").and_then(|t| t.as_array()) {
                let selected_idx = win.get("selected").and_then(|s| s.as_u64()).unwrap_or(1) as usize;
                if selected_idx >= 1 && selected_idx <= tabs.len() {
                    let (tab_title, _) = get_tab_title_and_url(&tabs[selected_idx - 1]);
                    let low = tab_title.to_lowercase();
                    if !low.is_empty() && (target.contains(&low) || low.contains(target)) {
                        selected_win = Some(win);
                        break;
                    }
                }
                for tab in tabs {
                    let (tab_title, _) = get_tab_title_and_url(tab);
                    let low = tab_title.to_lowercase();
                    if !low.is_empty() && (target.contains(&low) || low.contains(target)) {
                        selected_win = Some(win);
                        break;
                    }
                }
            }
            if selected_win.is_some() {
                break;
            }
        }
    }

    let win = selected_win.unwrap_or(&windows[0]);
    let raw_tabs = win.get("tabs")?.as_array()?;
    let selected_tab_idx = win.get("selected").and_then(|s| s.as_u64()).unwrap_or(1) as usize;

    let mut result = Vec::new();
    for (i, tab) in raw_tabs.iter().enumerate() {
        let is_hidden = tab.get("hidden").and_then(|h| h.as_bool()).unwrap_or(false);
        if is_hidden {
            continue;
        }

        let idx = i + 1;
        let active = idx == selected_tab_idx;
        let (title, url) = get_tab_title_and_url(tab);
        let tab_id = tab.get("id").and_then(|id| id.as_i64());

        result.push(BrowserTab {
            index: idx,
            title: if !title.is_empty() { title } else { format!("Tab {}", idx) },
            url,
            active,
            id: tab_id,
        });
    }

    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

/// Locate Firefox / Zen / Gecko recovery.jsonlz4 session store file
pub fn find_gecko_sessionstore_path(pid: i64, lower_class: &str) -> Option<PathBuf> {
    // 1. Try finding via /proc/{pid}/fd
    if pid > 0 {
        let fd_dir = PathBuf::from(format!("/proc/{}/fd", pid));
        if let Ok(entries) = std::fs::read_dir(&fd_dir) {
            for entry in entries.flatten() {
                if let Ok(target) = std::fs::read_link(entry.path()) {
                    let target_str = target.to_string_lossy();
                    if target_str.contains("sessionstore-backups")
                        || target_str.contains("places.sqlite")
                        || target_str.contains(".parentlock")
                    {
                        let mut curr = target.as_path();
                        while let Some(parent) = curr.parent() {
                            let rec = parent.join("sessionstore-backups").join("recovery.jsonlz4");
                            if rec.is_file() {
                                return Some(rec);
                            }
                            let rec2 = parent.join("recovery.jsonlz4");
                            if rec2.is_file() {
                                return Some(rec2);
                            }
                            curr = parent;
                        }
                    }
                }
            }
        }
    }

    // 2. Fallback: Search candidate directories for the browser class
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/astro".to_string());
    let mut patterns = Vec::new();

    if lower_class.contains("zen") {
        patterns.push(format!("{}/.zen/*/sessionstore-backups/recovery.jsonlz4", home));
        patterns.push(format!("{}/.var/app/app.zen_browser.zen/.zen/*/sessionstore-backups/recovery.jsonlz4", home));
    } else if lower_class.contains("floorp") {
        patterns.push(format!("{}/.floorp/*/sessionstore-backups/recovery.jsonlz4", home));
    } else if lower_class.contains("librewolf") {
        patterns.push(format!("{}/.librewolf/*/sessionstore-backups/recovery.jsonlz4", home));
    } else if lower_class.contains("waterfox") {
        patterns.push(format!("{}/.waterfox/*/sessionstore-backups/recovery.jsonlz4", home));
    } else if lower_class.contains("firefox") || lower_class.is_empty() || lower_class == "browser" {
        patterns.push(format!("{}/.config/mozilla/firefox/*/sessionstore-backups/recovery.jsonlz4", home));
        patterns.push(format!("{}/.mozilla/firefox/*/sessionstore-backups/recovery.jsonlz4", home));
        patterns.push(format!("{}/.var/app/org.mozilla.firefox/.mozilla/firefox/*/sessionstore-backups/recovery.jsonlz4", home));
    } else {
        return None;
    }

    let mut candidates = Vec::new();
    for pat in patterns {
        if let Some(star_idx) = pat.find('*') {
            let prefix = &pat[..star_idx];
            let suffix = &pat[star_idx + 1..];
            let prefix_path = Path::new(prefix);
            if prefix_path.is_dir() {
                if let Ok(entries) = std::fs::read_dir(prefix_path) {
                    for entry in entries.flatten() {
                        let candidate = entry.path().join(suffix.trim_start_matches('/'));
                        if candidate.is_file() {
                            let mtime = candidate
                                .metadata()
                                .and_then(|m| m.modified())
                                .unwrap_or(std::time::UNIX_EPOCH);
                            candidates.push((mtime, candidate));
                        }
                    }
                }
            }
        } else {
            let path = PathBuf::from(pat);
            if path.is_file() {
                let mtime = path
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::UNIX_EPOCH);
                candidates.push((mtime, path));
            }
        }
    }

    candidates.sort_by(|a, b| b.0.cmp(&a.0));
    candidates.into_iter().next().map(|(_, p)| p)
}

/// Parse Chromium SNSS session files (Session_* or Tabs_*) to extract active tabs
pub fn parse_chromium_session_file<P: AsRef<Path>>(path: P) -> Option<Vec<BrowserTab>> {
    let data = std::fs::read(path).ok()?;
    if data.len() < 8 || &data[..4] != b"SNSS" {
        return None;
    }

    let mut pos = 8;
    let mut tab_indices: BTreeMap<i32, i32> = BTreeMap::new();
    let mut closed_tabs: HashSet<i32> = HashSet::new();
    let mut tab_titles: HashMap<i32, String> = HashMap::new();

    while pos + 2 <= data.len() {
        let size = u16::from_le_bytes([data[pos], data[pos + 1]]) as usize;
        pos += 2;
        if pos + size > data.len() {
            break;
        }
        let payload = &data[pos..pos + size];
        pos += size;

        if payload.is_empty() {
            continue;
        }
        let cmd_id = payload[0];
        let p = &payload[1..];

        match cmd_id {
            0 if p.len() >= 8 => {
                let tab_id = i32::from_le_bytes([p[0], p[1], p[2], p[3]]);
                tab_indices.entry(tab_id).or_insert(0);
            }
            2 if p.len() >= 8 => {
                let tab_id = i32::from_le_bytes([p[0], p[1], p[2], p[3]]);
                let idx = i32::from_le_bytes([p[4], p[5], p[6], p[7]]);
                tab_indices.insert(tab_id, idx);
            }
            3 if p.len() >= 4 => {
                let tab_id = i32::from_le_bytes([p[0], p[1], p[2], p[3]]);
                closed_tabs.insert(tab_id);
            }
            6 if p.len() >= 8 => {
                let tab_id = i32::from_le_bytes([p[0], p[1], p[2], p[3]]);
                tab_indices.entry(tab_id).or_insert(0);
            }
            _ => {}
        }
    }

    let mut active_tabs: Vec<(i32, i32)> = tab_indices
        .into_iter()
        .filter(|(tid, _)| !closed_tabs.contains(tid))
        .collect();

    if active_tabs.is_empty() {
        return None;
    }

    active_tabs.sort_by_key(|&(_, idx)| idx);

    let tabs = active_tabs
        .into_iter()
        .enumerate()
        .map(|(i, (tid, _))| {
            let title = tab_titles.remove(&tid).unwrap_or_else(|| format!("Tab {}", i + 1));
            BrowserTab {
                index: i + 1,
                title,
                url: String::new(),
                active: i == 0,
                id: Some(tid as i64),
            }
        })
        .collect();

    Some(tabs)
}

/// Locate Chromium / Brave / Chrome Session_* file
pub fn find_chromium_session_path(_pid: i64, lower_class: &str) -> Option<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/astro".to_string());
    let mut base_dirs = Vec::new();

    if lower_class.contains("brave") {
        base_dirs.push(format!("{}/.config/BraveSoftware/Brave-Browser", home));
        base_dirs.push(format!("{}/.config/BraveSoftware/Brave-Origin", home));
    } else if lower_class.contains("chrome") {
        base_dirs.push(format!("{}/.config/google-chrome", home));
    } else if lower_class.contains("chromium") {
        base_dirs.push(format!("{}/.config/chromium", home));
    } else if lower_class.contains("vivaldi") {
        base_dirs.push(format!("{}/.config/vivaldi", home));
    } else if lower_class.contains("edge") {
        base_dirs.push(format!("{}/.config/microsoft-edge", home));
    }

    base_dirs.push(format!("{}/.config/BraveSoftware/Brave-Origin", home));
    base_dirs.push(format!("{}/.config/BraveSoftware/Brave-Browser", home));
    base_dirs.push(format!("{}/.config/google-chrome", home));

    let mut candidate_files = Vec::new();

    for base in base_dirs {
        let base_path = PathBuf::from(base);
        if !base_path.is_dir() {
            continue;
        }
        if let Ok(profile_entries) = std::fs::read_dir(&base_path) {
            for prof in profile_entries.flatten() {
                let sessions_dir = prof.path().join("Sessions");
                if sessions_dir.is_dir() {
                    if let Ok(sess_entries) = std::fs::read_dir(&sessions_dir) {
                        for sess_entry in sess_entries.flatten() {
                            let file_name = sess_entry.file_name();
                            let name_str = file_name.to_string_lossy();
                            if name_str.starts_with("Session_") {
                                let path = sess_entry.path();
                                let mtime = path
                                    .metadata()
                                    .and_then(|m| m.modified())
                                    .unwrap_or(std::time::UNIX_EPOCH);
                                candidate_files.push((mtime, path));
                            }
                        }
                    }
                }
            }
        }
    }

    candidate_files.sort_by(|a, b| b.0.cmp(&a.0));
    candidate_files.into_iter().next().map(|(_, p)| p)
}

/// Query the open tabs for a specific browser window based on its pid, class, and title.
/// Refreshed on-demand when the menu opens on a browser to minimize background resources.
pub fn get_browser_tabs_for_window(pid: i64, class: &str, title: &str) -> Vec<BrowserTab> {
    let lower_class = class.to_lowercase();

    // 1. Check PID-specific SHM file first (from native messaging host)
    if pid > 0 {
        let pid_shm = format!("/dev/shm/browser_tabs_{}.json", pid);
        if let Some(tabs) = read_tabs_from_path(&pid_shm) {
            if !tabs.is_empty() {
                return tabs;
            }
        }
    }

    // 2. Check general SHM file
    if let Some(tabs) = read_shm_tabs() {
        if !tabs.is_empty() {
            return tabs;
        }
    }

    // 3. Check Gecko browser session store (Firefox, Zen, Floorp, LibreWolf, Waterfox)
    let is_gecko = lower_class.contains("firefox")
        || lower_class.contains("zen")
        || lower_class.contains("floorp")
        || lower_class.contains("librewolf")
        || lower_class.contains("waterfox");

    if is_gecko {
        if let Some(session_path) = find_gecko_sessionstore_path(pid, &lower_class) {
            if let Ok(bytes) = std::fs::read(&session_path) {
                if let Ok(decompressed) = decompress_mozlz4(&bytes) {
                    if let Some(tabs) = parse_firefox_session_tabs(&decompressed, Some(title)) {
                        if !tabs.is_empty() {
                            return tabs;
                        }
                    }
                }
            }
        }
    }

    // 4. Check Chromium browser session store (Brave, Chrome, Chromium, Vivaldi, Edge)
    let is_chromium = lower_class.contains("brave")
        || lower_class.contains("chrome")
        || lower_class.contains("chromium")
        || lower_class.contains("vivaldi")
        || lower_class.contains("edge")
        || lower_class.contains("thorium")
        || lower_class.contains("opera");

    if is_chromium {
        if let Some(session_path) = find_chromium_session_path(pid, &lower_class) {
            if let Some(tabs) = parse_chromium_session_file(&session_path) {
                if !tabs.is_empty() {
                    return tabs;
                }
            }
        }
    }

    // 5. Graceful fallback: return 1 tab representing the current window
    vec![BrowserTab {
        index: 1,
        title: if !title.is_empty() {
            title.to_string()
        } else {
            "Tab 1".to_string()
        },
        url: String::new(),
        active: true,
        id: None,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_read_tabs_from_json_string() {
        // 1. Full tab objects
        let json_full = r#"[
            {"index": 1, "title": "GitHub - radial-dial", "url": "https://github.com", "active": true, "id": 101},
            {"index": 2, "title": "YouTube", "url": "https://youtube.com", "active": false, "id": 102}
        ]"#;
        let tabs = read_tabs_from_str(json_full).expect("should parse valid tabs");
        assert_eq!(tabs.len(), 2);
        assert_eq!(tabs[0].index, 1);
        assert_eq!(tabs[0].title, "GitHub - radial-dial");
        assert_eq!(tabs[0].url, "https://github.com");
        assert!(tabs[0].active);
        assert_eq!(tabs[0].id, Some(101));

        assert_eq!(tabs[1].index, 2);
        assert_eq!(tabs[1].title, "YouTube");
        assert_eq!(tabs[1].url, "https://youtube.com");
        assert!(!tabs[1].active);
        assert_eq!(tabs[1].id, Some(102));

        // 2. Minimal tab objects with defaults
        let json_minimal = r#"[
            {"index": 3, "title": "Docs"}
        ]"#;
        let tabs_min = read_tabs_from_str(json_minimal).expect("should parse minimal tab");
        assert_eq!(tabs_min.len(), 1);
        assert_eq!(tabs_min[0].index, 3);
        assert_eq!(tabs_min[0].title, "Docs");
        assert_eq!(tabs_min[0].url, "");
        assert!(!tabs_min[0].active);
        assert_eq!(tabs_min[0].id, None);

        // 3. Tab with omitted index (defaults to 0)
        let json_no_index = r#"[
            {"title": "Indexless Tab", "url": "https://example.com"}
        ]"#;
        let tabs_no_idx = read_tabs_from_str(json_no_index).expect("should parse tab with omitted index");
        assert_eq!(tabs_no_idx.len(), 1);
        assert_eq!(tabs_no_idx[0].index, 0);
        assert_eq!(tabs_no_idx[0].title, "Indexless Tab");

        // 4. Wrapped {"tabs": [...]}
        let json_wrapped = r#"{
            "tabs": [
                {"index": 1, "title": "Wrapped Tab", "url": "https://wrapped.org", "active": true, "id": 55}
            ]
        }"#;
        let tabs_wrapped = read_tabs_from_str(json_wrapped).expect("should parse wrapped tabs");
        assert_eq!(tabs_wrapped.len(), 1);
        assert_eq!(tabs_wrapped[0].title, "Wrapped Tab");
        assert_eq!(tabs_wrapped[0].id, Some(55));

        // 5. Empty array
        let json_empty = "[]";
        let tabs_empty = read_tabs_from_str(json_empty).expect("should parse empty array");
        assert!(tabs_empty.is_empty());

        // 6. Invalid JSON syntax
        assert!(read_tabs_from_str("{not valid json").is_none());

        // 7. Non-tab object
        assert!(read_tabs_from_str(r#"{"unexpected": 42}"#).is_none());
    }

    #[test]
    fn test_native_host_protocol_framing() {
        let payload = br#"{"status":"ok","count":3}"#;

        // 1. Test encode and decode framing length
        let encoded = encode_framed_message(payload);
        assert_eq!(encoded.len(), 4 + payload.len());
        let mut len_bytes = [0u8; 4];
        len_bytes.copy_from_slice(&encoded[..4]);
        let decoded_len = decode_frame_length(len_bytes);
        assert_eq!(decoded_len as usize, payload.len());
        assert_eq!(&encoded[4..], payload);

        // 2. Test write_framed_message and read_framed_message
        let mut stream = Vec::new();
        write_framed_message(&mut stream, payload).expect("write framed message failed");
        assert_eq!(stream, encoded);

        let mut cursor = Cursor::new(stream);
        let read_back = read_framed_message(&mut cursor)
            .expect("read framed message failed")
            .expect("expected payload");
        assert_eq!(read_back, payload);

        // 3. Test multiple sequential messages
        let msg1 = b"first message";
        let msg2 = b"second message";
        let mut multi_buf = Vec::new();
        write_framed_message(&mut multi_buf, msg1).unwrap();
        write_framed_message(&mut multi_buf, msg2).unwrap();

        let mut multi_cursor = Cursor::new(multi_buf);
        let r1 = read_framed_message(&mut multi_cursor).unwrap().unwrap();
        assert_eq!(r1, msg1);
        let r2 = read_framed_message(&mut multi_cursor).unwrap().unwrap();
        assert_eq!(r2, msg2);
        let r3 = read_framed_message(&mut multi_cursor).unwrap();
        assert!(r3.is_none(), "expected clean EOF after last message");

        // 4. Test clean EOF on empty stream
        let mut empty_cursor = Cursor::new(Vec::new());
        assert!(read_framed_message(&mut empty_cursor).unwrap().is_none());

        // 5. Test truncated header (1..3 bytes)
        let truncated_header = vec![1, 0];
        let mut trunc_cursor = Cursor::new(truncated_header);
        assert!(read_framed_message(&mut trunc_cursor).is_err());

        // 6. Test truncated payload (header specifies 10 bytes, but only 3 provided)
        let mut bad_payload = 10u32.to_le_bytes().to_vec();
        bad_payload.extend_from_slice(b"abc");
        let mut bad_cursor = Cursor::new(bad_payload);
        assert!(read_framed_message(&mut bad_cursor).is_err());
    }

    #[test]
    fn test_read_shm_tabs_missing_file() {
        let non_existent = Path::new("/tmp/radial_non_existent_tabs_test_12345.json");
        if non_existent.exists() {
            let _ = std::fs::remove_file(non_existent);
        }
        assert!(read_tabs_from_path(non_existent).is_none());
    }

    #[test]
    fn test_atomic_write_and_read_tabs() {
        let temp_dir = std::env::temp_dir().join(format!("radial_dial_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let test_file = temp_dir.join("browser_tabs.json");

        let test_tabs = vec![
            BrowserTab {
                index: 1,
                title: "Tab 1".to_string(),
                url: "https://t1.org".to_string(),
                active: true,
                id: Some(1),
            },
            BrowserTab {
                index: 2,
                title: "Tab 2".to_string(),
                url: "https://t2.org".to_string(),
                active: false,
                id: Some(2),
            },
        ];

        write_tabs_atomically(&test_file, &test_tabs).expect("atomic write should succeed");
        let loaded = read_tabs_from_path(&test_file).expect("should read back saved tabs");
        assert_eq!(loaded, test_tabs);

        let _ = std::fs::remove_file(&test_file);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_watch_tabs_notification() {
        let temp_dir = std::env::temp_dir().join(format!("radial_dial_watch_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let test_file = temp_dir.join("browser_tabs.json");

        let initial_tabs = vec![
            BrowserTab {
                index: 1,
                title: "Initial Tab".to_string(),
                url: "https://init.org".to_string(),
                active: true,
                id: Some(10),
            },
        ];
        write_tabs_atomically(&test_file, &initial_tabs).expect("initial write");

        let (tx, mut rx) = tokio::sync::watch::channel(Vec::new());
        let test_file_clone = test_file.clone();
        let watch_handle = tokio::spawn(async move {
            watch_tabs_path(test_file_clone, tx).await;
        });

        // Give inotify watcher time to establish and deliver initial value
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        {
            let current = rx.borrow_and_update().clone();
            assert_eq!(current, initial_tabs);
        }

        // Now write updated tabs atomically
        let updated_tabs = vec![
            BrowserTab {
                index: 1,
                title: "Updated Tab 1".to_string(),
                url: "https://updated1.org".to_string(),
                active: false,
                id: Some(10),
            },
            BrowserTab {
                index: 2,
                title: "Updated Tab 2".to_string(),
                url: "https://updated2.org".to_string(),
                active: true,
                id: Some(20),
            },
        ];
        write_tabs_atomically(&test_file, &updated_tabs).expect("updated write");

        // Wait for change notification
        let changed = tokio::time::timeout(std::time::Duration::from_millis(500), rx.changed()).await;
        assert!(changed.is_ok(), "timeout waiting for inotify watch change");

        let received = rx.borrow().clone();
        assert_eq!(received, updated_tabs);

        watch_handle.abort();
        let _ = std::fs::remove_file(&test_file);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_mozlz4_decompression_and_session_parsing() {
        // Create mock Firefox session JSON
        let mock_session = serde_json::json!({
            "windows": [
                {
                    "selected": 2,
                    "tabs": [
                        {
                            "entries": [
                                {"title": "Tab 1 - Google", "url": "https://google.com"}
                            ]
                        },
                        {
                            "entries": [
                                {"title": "Tab 2 - YouTube", "url": "https://youtube.com"}
                            ]
                        },
                        {
                            "entries": [
                                {"title": "Tab 3 - GitHub", "url": "https://github.com"}
                            ]
                        }
                    ]
                }
            ]
        });
        let raw_json = serde_json::to_vec(&mock_session).unwrap();

        // Test parsing uncompressed JSON bytes directly
        let parsed = parse_firefox_session_tabs(&raw_json, Some("YouTube")).expect("should parse session tabs");
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0].title, "Tab 1 - Google");
        assert_eq!(parsed[0].index, 1);
        assert!(!parsed[0].active);

        assert_eq!(parsed[1].title, "Tab 2 - YouTube");
        assert_eq!(parsed[1].index, 2);
        assert!(parsed[1].active); // selected is 2

        assert_eq!(parsed[2].title, "Tab 3 - GitHub");
        assert_eq!(parsed[2].index, 3);
        assert!(!parsed[2].active);

        // Test with live recovery.jsonlz4 on system if present
        if let Some(session_path) = find_gecko_sessionstore_path(0, "firefox") {
            if let Ok(bytes) = std::fs::read(&session_path) {
                if let Ok(decompressed) = decompress_mozlz4(&bytes) {
                    let live_tabs = parse_firefox_session_tabs(&decompressed, None);
                    assert!(live_tabs.is_some(), "should parse live firefox session");
                    let tabs = live_tabs.unwrap();
                    assert!(!tabs.is_empty(), "live firefox session has tabs");
                    println!("Successfully loaded {} live Firefox tabs!", tabs.len());
                }
            }
        }
    }

    #[test]
    fn test_get_browser_tabs_for_window_fallback() {
        let tabs = get_browser_tabs_for_window(-1, "nonexistent-browser-xyz", "My Test Page");
        assert_eq!(tabs.len(), 1);
        assert_eq!(tabs[0].title, "My Test Page");
        assert_eq!(tabs[0].index, 1);
        assert!(tabs[0].active);
    }
}
