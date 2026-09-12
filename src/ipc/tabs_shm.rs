// src/ipc/tabs_shm.rs

use crate::state::menu::BrowserTab;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::io::{Read, Write};
use std::path::Path;

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
}
