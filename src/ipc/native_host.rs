// src/ipc/native_host.rs
//! Standalone WebExtension Native Messaging Host (`radial-tabs-host`).
//!
//! Receives tab synchronization messages from browser extensions (Firefox, Chrome, Zen)
//! via stdin, parses the tab list, and writes it atomically to `/dev/shm/browser_tabs.json`.
//!
//! Protocol:
//! - 4-byte unsigned integer (little-endian) message length N
//! - N bytes of UTF-8 JSON message
//! - Response: 4-byte unsigned int (little-endian) length M + M bytes of UTF-8 JSON response

use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;

pub const DEFAULT_SHM_PATH: &str = "/dev/shm/browser_tabs.json";

/// Read a length-prefixed message from a reader.
/// Returns Ok(None) on clean EOF at frame boundary.
/// Returns Err(UnexpectedEof) on truncated header or payload.
pub fn read_framed_message<R: Read>(reader: &mut R) -> io::Result<Option<Vec<u8>>> {
    let mut len_buf = [0u8; 4];
    let mut n = 0;
    while n < 4 {
        match reader.read(&mut len_buf[n..])? {
            0 => {
                if n == 0 {
                    return Ok(None);
                } else {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "Unexpected EOF reading message length header",
                    ));
                }
            }
            bytes_read => n += bytes_read,
        }
    }

    let len = u32::from_le_bytes(len_buf) as usize;
    let mut payload = vec![0u8; len];
    reader.read_exact(&mut payload)?;
    Ok(Some(payload))
}

/// Write a length-prefixed message to a writer and flush.
pub fn write_framed_message<W: Write>(writer: &mut W, payload: &[u8]) -> io::Result<()> {
    let len = payload.len() as u32;
    writer.write_all(&len.to_le_bytes())?;
    writer.write_all(payload)?;
    writer.flush()?;
    Ok(())
}

/// Atomically write tabs data to target path (write to .tmp.<pid> then rename).
pub fn write_tabs_atomically(target_path: &Path, data: &[u8]) -> io::Result<()> {
    if let Some(parent) = target_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }
    let tmp_path = format!("{}.tmp.{}", target_path.display(), std::process::id());
    let tmp = Path::new(&tmp_path);
    fs::write(tmp, data)?;
    fs::rename(tmp, target_path)?;
    Ok(())
}

/// Process a single incoming raw payload, returning the number of tabs written.
pub fn process_incoming_payload(payload: &[u8], target_path: &Path) -> Result<usize, String> {
    let msg: serde_json::Value =
        serde_json::from_slice(payload).map_err(|e| format!("JSON parse error: {e}"))?;

    let (tabs_val, count) = match msg {
        serde_json::Value::Array(arr) => {
            let count = arr.len();
            (serde_json::Value::Array(arr), count)
        }
        serde_json::Value::Object(mut obj) => {
            if let Some(serde_json::Value::Array(arr)) = obj.remove("tabs") {
                let count = arr.len();
                (serde_json::Value::Array(arr), count)
            } else {
                (serde_json::Value::Array(Vec::new()), 0)
            }
        }
        _ => (serde_json::Value::Array(Vec::new()), 0),
    };

    let serialized = serde_json::to_vec(&tabs_val)
        .map_err(|e| format!("JSON serialization error: {e}"))?;

    write_tabs_atomically(target_path, &serialized)
        .map_err(|e| format!("Atomic write error: {e}"))?;

    // Also write to PID-specific path so multiple browsers / windows maintain separate tabs
    let ppid = unsafe { libc::getppid() };
    if ppid > 1 {
        let pid_path = format!("/dev/shm/browser_tabs_{}.json", ppid);
        let _ = write_tabs_atomically(Path::new(&pid_path), &serialized);
    }

    Ok(count)
}

/// Run the native host message loop processing incoming frames from `reader` and writing responses to `writer`.
pub fn run_native_host_loop<R: Read, W: Write>(
    mut reader: R,
    mut writer: W,
    target_path: &Path,
) -> io::Result<()> {
    while let Some(payload) = read_framed_message(&mut reader)? {
        match process_incoming_payload(&payload, target_path) {
            Ok(count) => {
                let resp = serde_json::json!({
                    "status": "ok",
                    "count": count,
                });
                let resp_bytes = serde_json::to_vec(&resp)
                    .unwrap_or_else(|_| b"{\"status\":\"ok\"}".to_vec());
                write_framed_message(&mut writer, &resp_bytes)?;
            }
            Err(err_msg) => {
                let resp = serde_json::json!({
                    "status": "error",
                    "message": err_msg,
                });
                let resp_bytes = serde_json::to_vec(&resp)
                    .unwrap_or_else(|_| b"{\"status\":\"error\"}".to_vec());
                write_framed_message(&mut writer, &resp_bytes)?;
            }
        }
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && (args[1] == "--version" || args[1] == "-v") {
        println!("radial-tabs-host 0.1.0");
        return;
    }

    let shm_path_env = std::env::var("RADIAL_TABS_SHM_PATH")
        .unwrap_or_else(|_| DEFAULT_SHM_PATH.to_string());
    let target_path = Path::new(&shm_path_env);

    let stdin = io::stdin();
    let stdout = io::stdout();

    let mut reader = stdin.lock();
    let mut writer = stdout.lock();

    if let Err(e) = run_native_host_loop(&mut reader, &mut writer, target_path) {
        eprintln!("radial-tabs-host error: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_native_host_framing_roundtrip() {
        let payload = b"{\"hello\":\"world\"}";
        let mut stream = Vec::new();
        write_framed_message(&mut stream, payload).expect("write failed");

        let len = u32::from_le_bytes([stream[0], stream[1], stream[2], stream[3]]) as usize;
        assert_eq!(len, payload.len());
        assert_eq!(&stream[4..], payload);

        let mut cursor = Cursor::new(stream);
        let read = read_framed_message(&mut cursor).expect("read failed").expect("some");
        assert_eq!(read, payload);
    }

    #[test]
    fn test_native_host_loop_array_message() {
        let temp_dir = std::env::temp_dir().join(format!("radial_host_test_1_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let shm_file = temp_dir.join("browser_tabs.json");

        let tabs_json = r#"[
            {"index": 1, "title": "Tab 1", "url": "https://a.com", "active": true, "id": 10},
            {"index": 2, "title": "Tab 2", "url": "https://b.com", "active": false, "id": 20}
        ]"#;

        let mut in_bytes = Vec::new();
        write_framed_message(&mut in_bytes, tabs_json.as_bytes()).unwrap();

        let reader = Cursor::new(in_bytes);
        let mut writer = Vec::new();

        run_native_host_loop(reader, &mut writer, &shm_file).expect("loop should succeed");

        // Verify written file
        let written_content = fs::read_to_string(&shm_file).expect("shm file should exist");
        let parsed: serde_json::Value = serde_json::from_str(&written_content).unwrap();
        assert_eq!(parsed.as_array().unwrap().len(), 2);

        // Verify response
        let mut out_cursor = Cursor::new(writer);
        let resp_bytes = read_framed_message(&mut out_cursor).unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_slice(&resp_bytes).unwrap();
        assert_eq!(resp["status"], "ok");
        assert_eq!(resp["count"], 2);

        let _ = fs::remove_file(&shm_file);
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_native_host_loop_wrapped_message() {
        let temp_dir = std::env::temp_dir().join(format!("radial_host_test_2_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let shm_file = temp_dir.join("browser_tabs.json");

        let wrapped_json = r#"{
            "tabs": [
                {"index": 1, "title": "Single Tab", "url": "https://single.org", "active": true}
            ]
        }"#;

        let mut in_bytes = Vec::new();
        write_framed_message(&mut in_bytes, wrapped_json.as_bytes()).unwrap();

        let reader = Cursor::new(in_bytes);
        let mut writer = Vec::new();

        run_native_host_loop(reader, &mut writer, &shm_file).expect("loop should succeed");

        let written_content = fs::read_to_string(&shm_file).expect("shm file should exist");
        let parsed: serde_json::Value = serde_json::from_str(&written_content).unwrap();
        assert_eq!(parsed.as_array().unwrap().len(), 1);

        let mut out_cursor = Cursor::new(writer);
        let resp_bytes = read_framed_message(&mut out_cursor).unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_slice(&resp_bytes).unwrap();
        assert_eq!(resp["status"], "ok");
        assert_eq!(resp["count"], 1);

        let _ = fs::remove_file(&shm_file);
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_native_host_loop_invalid_json() {
        let temp_dir = std::env::temp_dir().join(format!("radial_host_test_3_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let shm_file = temp_dir.join("browser_tabs.json");

        let mut in_bytes = Vec::new();
        write_framed_message(&mut in_bytes, b"{not valid json").unwrap();

        let reader = Cursor::new(in_bytes);
        let mut writer = Vec::new();

        run_native_host_loop(reader, &mut writer, &shm_file).expect("loop handles error gracefully");

        // Response should be status error
        let mut out_cursor = Cursor::new(writer);
        let resp_bytes = read_framed_message(&mut out_cursor).unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_slice(&resp_bytes).unwrap();
        assert_eq!(resp["status"], "error");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_native_host_loop_clean_eof() {
        let temp_dir = std::env::temp_dir().join(format!("radial_host_test_4_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let shm_file = temp_dir.join("browser_tabs.json");

        let reader = Cursor::new(Vec::new());
        let mut writer = Vec::new();

        let res = run_native_host_loop(reader, &mut writer, &shm_file);
        assert!(res.is_ok());
        assert!(writer.is_empty());

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
