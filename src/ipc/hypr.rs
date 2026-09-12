#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct HyprCursorPos {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct HyprWorkspace {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct HyprWindow {
    pub address: String,
    pub class: String,
    #[serde(rename = "initialClass", default)]
    pub initial_class: String,
    pub title: String,
    pub pid: i64,
    pub workspace: HyprWorkspace,
    #[serde(default)]
    pub at: Vec<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct HyprClient {
    pub address: String,
    pub class: String,
    pub title: String,
    pub pid: i64,
    pub workspace: HyprWorkspace,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct HyprContext {
    pub cursor: HyprCursorPos,
    pub window: HyprWindow,
    pub clients: Vec<HyprClient>,
}

/// Socket Discovery (port of hypr_ipc.py get_hypr_socket())
/// 1. Prefer $XDG_RUNTIME_DIR/hypr/$HYPRLAND_INSTANCE_SIGNATURE/.socket.sock
/// 2. Fallback: scan $XDG_RUNTIME_DIR/hypr/*/socket.sock, pick newest mtime
/// $XDG_RUNTIME_DIR defaults to /run/user/{uid} if not set
pub fn get_hypr_socket() -> Option<String> {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| {
        let uid = unsafe { libc::getuid() };
        format!("/run/user/{}", uid)
    });

    // 1. Prefer $XDG_RUNTIME_DIR/hypr/$HYPRLAND_INSTANCE_SIGNATURE/.socket.sock
    if let Ok(sig) = std::env::var("HYPRLAND_INSTANCE_SIGNATURE") {
        if !sig.is_empty() {
            let sock = Path::new(&runtime_dir).join("hypr").join(&sig).join(".socket.sock");
            if sock.exists() {
                return Some(sock.to_string_lossy().to_string());
            }
            let sock_alt = Path::new(&runtime_dir).join("hypr").join(&sig).join("socket.sock");
            if sock_alt.exists() {
                return Some(sock_alt.to_string_lossy().to_string());
            }
        }
    }

    // 2. Fallback: scan $XDG_RUNTIME_DIR/hypr/*/socket.sock or .socket.sock, pick newest mtime
    let hypr_dir = Path::new(&runtime_dir).join("hypr");
    if hypr_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&hypr_dir) {
            let mut candidates: Vec<(SystemTime, PathBuf)> = Vec::new();
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let s1 = path.join(".socket.sock");
                    let s2 = path.join("socket.sock");
                    let sock_path = if s1.exists() {
                        Some(s1)
                    } else if s2.exists() {
                        Some(s2)
                    } else {
                        None
                    };

                    if let Some(sp) = sock_path {
                        let mtime = sp
                            .metadata()
                            .and_then(|m| m.modified())
                            .unwrap_or(UNIX_EPOCH);
                        candidates.push((mtime, sp));
                    }
                }
            }

            if !candidates.is_empty() {
                candidates.sort_by(|a, b| b.0.cmp(&a.0));
                return Some(candidates[0].1.to_string_lossy().to_string());
            }
        }
    }

    None
}

/// Raw query to Hyprland UNIX socket.
/// Times out after 150ms.
pub async fn raw_query(cmd: &str) -> anyhow::Result<String> {
    let sock_path = get_hypr_socket().ok_or_else(|| anyhow::anyhow!("Hyprland socket not found"))?;

    let fut = async {
        let mut stream = UnixStream::connect(&sock_path)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to connect to Hyprland socket at {}: {}", sock_path, e))?;

        stream
            .write_all(cmd.as_bytes())
            .await
            .map_err(|e| anyhow::anyhow!("Failed to write to Hyprland socket: {}", e))?;

        let mut buf = Vec::new();
        stream
            .read_to_end(&mut buf)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to read from Hyprland socket: {}", e))?;

        String::from_utf8(buf).map_err(|e| anyhow::anyhow!("Invalid UTF-8 from Hyprland socket: {}", e))
    };

    match tokio::time::timeout(Duration::from_millis(150), fut).await {
        Ok(res) => res,
        Err(_) => Err(anyhow::anyhow!("Hyprland socket timeout")),
    }
}

/// Send a command to Hyprland socket and return parsed JSON.
/// Times out after 150ms to match original Python behavior.
pub async fn query(cmd: &str) -> anyhow::Result<serde_json::Value> {
    let raw = raw_query(cmd).await?;
    serde_json::from_str(&raw).map_err(|e| anyhow::anyhow!("JSON parse error: {}", e))
}

/// Send a hyprctl dispatch command (no return value needed)
pub async fn dispatch(cmd: &str) -> anyhow::Result<()> {
    let full_cmd = if cmd.starts_with("/dispatch ") {
        cmd.to_string()
    } else if let Some(stripped) = cmd.strip_prefix("dispatch ") {
        format!("/dispatch {}", stripped)
    } else if cmd.starts_with('/') {
        cmd.to_string()
    } else {
        format!("/dispatch {}", cmd)
    };

    let resp = raw_query(&full_cmd).await?;
    if resp.starts_with("error:") {
        log::warn!("Hyprland dispatch error: {}", resp.trim());
    }
    Ok(())
}

/// Fetch cursor pos, active window, and all clients concurrently via tokio::join!
pub async fn get_context() -> anyhow::Result<HyprContext> {
    if get_hypr_socket().is_none() {
        return Err(anyhow::anyhow!("Hyprland socket not found"));
    }

    let (cursor_res, window_res, clients_res) = tokio::join!(
        query("j/cursorpos"),
        query("j/activewindow"),
        query("j/clients"),
    );

    // If all failed, return the first error encountered
    if cursor_res.is_err() && window_res.is_err() && clients_res.is_err() {
        return Err(cursor_res.unwrap_err());
    }

    let cursor: HyprCursorPos = cursor_res
        .ok()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    let window: HyprWindow = window_res
        .ok()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    let clients: Vec<HyprClient> = clients_res
        .ok()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    Ok(HyprContext {
        cursor,
        window,
        clients,
    })
}

/// Focus a window by address (port of focusWindow in RadialMenuActions.qml)
pub async fn focus_window(address: &str, workspace_name: &str) -> anyhow::Result<()> {
    // If workspace_name starts with "special:", use special workspace dispatch
    // First switch workspace, then focus window address
    // Use hyprctl dispatch focuswindow address:{address}
    if !workspace_name.is_empty() {
        if workspace_name.starts_with("special:") {
            let special_name = workspace_name.trim_start_matches("special:");
            // Modern Hyprland (Lua) dispatch
            let _ = dispatch(&format!("hl.dsp.focus({{ workspace = \"{}\" }})", workspace_name)).await;
            // Legacy/standard Hyprland dispatch
            let _ = if special_name.is_empty() {
                dispatch("togglespecialworkspace").await
            } else {
                dispatch(&format!("togglespecialworkspace {}", special_name)).await
            };
        } else {
            // Modern Hyprland (Lua) dispatch
            let _ = dispatch(&format!("hl.dsp.focus({{ workspace = {} }})", workspace_name)).await;
            // Legacy/standard Hyprland dispatch
            let _ = dispatch(&format!("workspace {}", workspace_name)).await;
        }
    }

    let _ = dispatch(&format!("hl.dsp.focus({{ window = \"address:{}\" }})", address)).await;
    dispatch(&format!("focuswindow address:{}", address)).await
}

/// Helper to query active window
pub async fn get_active_window() -> anyhow::Result<HyprWindow> {
    let val = query("j/activewindow").await?;
    serde_json::from_value(val).map_err(|e| anyhow::anyhow!("JSON parse error: {}", e))
}

/// Helper to query cursor position
pub async fn get_cursor_pos() -> anyhow::Result<HyprCursorPos> {
    let val = query("j/cursorpos").await?;
    serde_json::from_value(val).map_err(|e| anyhow::anyhow!("JSON parse error: {}", e))
}

/// Helper to query all clients
pub async fn get_clients() -> anyhow::Result<Vec<HyprClient>> {
    let val = query("j/clients").await?;
    serde_json::from_value(val).map_err(|e| anyhow::anyhow!("JSON parse error: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn test_socket_path_env_override() {
        let _guard = ENV_LOCK.lock().unwrap();
        // Set env vars and verify path construction
        // (don't need actual socket to exist for path computation test)
        std::env::set_var("XDG_RUNTIME_DIR", "/tmp/test_runtime");
        std::env::set_var("HYPRLAND_INSTANCE_SIGNATURE", "test123");
        let path = get_hypr_socket();
        // Either Some(path containing "test123") or None if file doesn't exist
        // Just verify the function doesn't panic
        let _ = path;
    }

    #[test]
    fn test_hypr_window_deserialize() {
        let json = r#"{"address":"0x123","class":"kitty","initialClass":"","title":"~","pid":1234,"workspace":{"id":1,"name":"1"},"at":[100,200]}"#;
        let w: HyprWindow = serde_json::from_str(json).unwrap();
        assert_eq!(w.class, "kitty");
        assert_eq!(w.workspace.id, 1);
    }

    #[test]
    fn test_hypr_client_list_deserialize() {
        let json = r#"[{"address":"0xabc","class":"firefox","title":"Mozilla Firefox","pid":5678,"workspace":{"id":2,"name":"2"}}]"#;
        let clients: Vec<HyprClient> = serde_json::from_str(json).unwrap();
        assert_eq!(clients.len(), 1);
        assert_eq!(clients[0].class, "firefox");
    }

    #[test]
    fn test_hypr_cursor_pos_deserialize() {
        let json = r#"{"x": 1920.5, "y": 1080.0}"#;
        let c: HyprCursorPos = serde_json::from_str(json).unwrap();
        assert_eq!(c.x, 1920.5);
        assert_eq!(c.y, 1080.0);
    }

    #[test]
    fn test_hypr_empty_window_deserialize() {
        let json = r#"{}"#;
        let w: HyprWindow = serde_json::from_str(json).unwrap();
        assert_eq!(w.address, "");
        assert_eq!(w.class, "");
        assert_eq!(w.title, "");
        assert_eq!(w.pid, 0);
        assert_eq!(w.workspace.id, 0);
        assert!(w.at.is_empty());
    }

    #[test]
    fn test_socket_discovery_fallback() {
        let _guard = ENV_LOCK.lock().unwrap();
        let unique_id = format!("hypr_test_{}", std::process::id());
        let temp_dir = std::env::temp_dir().join(&unique_id);
        let older_dir = temp_dir.join("hypr").join("instance_older");
        let newer_dir = temp_dir.join("hypr").join("instance_newer");

        std::fs::create_dir_all(&older_dir).unwrap();
        std::fs::create_dir_all(&newer_dir).unwrap();

        let old_sock = older_dir.join(".socket.sock");
        let new_sock = newer_dir.join(".socket.sock");

        std::fs::write(&old_sock, b"").unwrap();
        // Small sleep or explicit filetime to ensure distinct timestamps
        std::thread::sleep(std::time::Duration::from_millis(50));
        std::fs::write(&new_sock, b"").unwrap();

        std::env::set_var("XDG_RUNTIME_DIR", temp_dir.to_str().unwrap());
        std::env::remove_var("HYPRLAND_INSTANCE_SIGNATURE");

        let found = get_hypr_socket();
        assert_eq!(found, Some(new_sock.to_string_lossy().to_string()));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_mock_socket_query_and_dispatch() {
        let _guard = ENV_LOCK.lock().unwrap();
        let unique_id = format!("hypr_sock_test_{}", std::process::id());
        let temp_dir = std::env::temp_dir().join(&unique_id);
        let instance_dir = temp_dir.join("hypr").join("mock_instance");
        std::fs::create_dir_all(&instance_dir).unwrap();
        let sock_path = instance_dir.join(".socket.sock");

        let listener = tokio::net::UnixListener::bind(&sock_path).unwrap();

        // Spawn mock Hyprland socket handler
        tokio::spawn(async move {
            loop {
                if let Ok((mut stream, _)) = listener.accept().await {
                    tokio::spawn(async move {
                        let mut buf = [0u8; 1024];
                        if let Ok(n) = stream.read(&mut buf).await {
                            let req = String::from_utf8_lossy(&buf[..n]);
                            let resp = if req.contains("cursorpos") {
                                r#"{"x": 123.0, "y": 456.0}"#
                            } else if req.contains("activewindow") {
                                r#"{"address": "0x42", "class": "kitty", "initialClass": "kitty", "title": "myterm", "pid": 999, "workspace": {"id": 1, "name": "1"}, "at": [10, 20]}"#
                            } else if req.contains("clients") {
                                r#"[{"address": "0x42", "class": "kitty", "title": "myterm", "pid": 999, "workspace": {"id": 1, "name": "1"}}]"#
                            } else {
                                "ok"
                            };
                            let _ = stream.write_all(resp.as_bytes()).await;
                        }
                    });
                } else {
                    break;
                }
            }
        });

        std::env::set_var("XDG_RUNTIME_DIR", temp_dir.to_str().unwrap());
        std::env::set_var("HYPRLAND_INSTANCE_SIGNATURE", "mock_instance");

        // Test query
        let cursor_val = query("j/cursorpos").await.unwrap();
        let cursor: HyprCursorPos = serde_json::from_value(cursor_val).unwrap();
        assert_eq!(cursor.x, 123.0);
        assert_eq!(cursor.y, 456.0);

        // Test get_context
        let ctx = get_context().await.unwrap();
        assert_eq!(ctx.cursor.x, 123.0);
        assert_eq!(ctx.window.class, "kitty");
        assert_eq!(ctx.window.title, "myterm");
        assert_eq!(ctx.clients.len(), 1);
        assert_eq!(ctx.clients[0].address, "0x42");

        // Test dispatch & focus_window
        let disp = dispatch("workspace 2").await;
        assert!(disp.is_ok());

        let focus = focus_window("0x42", "special:scratchpad").await;
        assert!(focus.is_ok());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_socket_not_found() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("XDG_RUNTIME_DIR", "/nonexistent_runtime_dir_12345");
        std::env::remove_var("HYPRLAND_INSTANCE_SIGNATURE");

        let res = query("j/cursorpos").await;
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "Hyprland socket not found");

        let ctx_res = get_context().await;
        assert!(ctx_res.is_err());
        assert_eq!(ctx_res.unwrap_err().to_string(), "Hyprland socket not found");
    }

    #[tokio::test]
    async fn test_socket_timeout() {
        let _guard = ENV_LOCK.lock().unwrap();
        let unique_id = format!("hypr_timeout_test_{}", std::process::id());
        let temp_dir = std::env::temp_dir().join(&unique_id);
        let instance_dir = temp_dir.join("hypr").join("timeout_instance");
        std::fs::create_dir_all(&instance_dir).unwrap();
        let sock_path = instance_dir.join(".socket.sock");

        let listener = tokio::net::UnixListener::bind(&sock_path).unwrap();

        tokio::spawn(async move {
            if let Ok((_stream, _)) = listener.accept().await {
                // Sleep longer than 150ms timeout
                tokio::time::sleep(Duration::from_millis(300)).await;
            }
        });

        std::env::set_var("XDG_RUNTIME_DIR", temp_dir.to_str().unwrap());
        std::env::set_var("HYPRLAND_INSTANCE_SIGNATURE", "timeout_instance");

        let res = query("j/cursorpos").await;
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "Hyprland socket timeout");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_live_hyprland_if_present() {
        let _guard = ENV_LOCK.lock().unwrap();
        if std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok() && get_hypr_socket().is_some() {
            let ctx = get_context().await;
            assert!(ctx.is_ok(), "Live context fetch failed: {:?}", ctx.err());
            let ctx = ctx.unwrap();
            assert!(ctx.cursor.x >= 0.0);
            assert!(ctx.cursor.y >= 0.0);
        }
    }
}
