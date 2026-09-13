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
    #[serde(default)]
    pub size: Vec<i64>,
    #[serde(default)]
    pub floating: bool,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub mapped: bool,
    #[serde(default)]
    pub hidden: bool,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct HyprClient {
    pub address: String,
    pub class: String,
    #[serde(rename = "initialClass", default)]
    pub initial_class: String,
    pub title: String,
    pub pid: i64,
    pub workspace: HyprWorkspace,
    #[serde(default)]
    pub at: Vec<i64>,
    #[serde(default)]
    pub size: Vec<i64>,
    #[serde(default)]
    pub floating: bool,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub mapped: bool,
    #[serde(default)]
    pub hidden: bool,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct HyprMonitor {
    pub id: i64,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub scale: f64,
    pub transform: i32,
    pub focused: bool,
    #[serde(rename = "activeWorkspace", default)]
    pub active_workspace: HyprWorkspace,
    #[serde(rename = "specialWorkspace", default)]
    pub special_workspace: HyprWorkspace,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct HyprContext {
    pub cursor: HyprCursorPos,
    pub window: HyprWindow,
    pub clients: Vec<HyprClient>,
    pub monitors: Vec<HyprMonitor>,
    pub target_monitor_name: Option<String>,
    pub is_special_workspace: bool,
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

/// Resolve the target monitor containing the cursor, and the surface-local cursor coordinates
pub fn resolve_target_monitor_and_local_cursor<'a>(
    raw_cursor: &HyprCursorPos,
    monitors: &'a [HyprMonitor],
) -> (Option<&'a HyprMonitor>, HyprCursorPos) {
    if monitors.is_empty() {
        return (None, raw_cursor.clone());
    }

    let target = monitors
        .iter()
        .find(|m| {
            let (phys_w, phys_h) = if m.transform % 2 == 1 {
                (m.height, m.width)
            } else {
                (m.width, m.height)
            };
            let scale = if m.scale > 0.0 { m.scale } else { 1.0 };
            let log_w = (phys_w as f64 / scale).round() as f32;
            let log_h = (phys_h as f64 / scale).round() as f32;

            let min_x = m.x as f32;
            let max_x = m.x as f32 + log_w;
            let min_y = m.y as f32;
            let max_y = m.y as f32 + log_h;

            raw_cursor.x >= min_x && raw_cursor.x < max_x && raw_cursor.y >= min_y && raw_cursor.y < max_y
        })
        .or_else(|| monitors.iter().find(|m| m.focused))
        .or_else(|| monitors.first());

    let local_cursor = if let Some(m) = target {
        HyprCursorPos {
            x: (raw_cursor.x - m.x as f32).max(0.0),
            y: (raw_cursor.y - m.y as f32).max(0.0),
        }
    } else {
        raw_cursor.clone()
    };

    (target, local_cursor)
}

/// Resolve the effective window context based on cursor position, active workspace, and clients
pub fn resolve_effective_window(
    raw_cursor: &HyprCursorPos,
    active_window: &HyprWindow,
    clients: &[HyprClient],
    active_workspace_id: i64,
    is_special_active: bool,
) -> HyprWindow {
    let is_pip = |w_class: &str, w_title: &str| -> bool {
        crate::state::context::is_pip_window(w_class, w_title)
    };

    // Look for client directly under raw_cursor on the active workspace
    let mut window_under_cursor: Option<HyprWindow> = None;
    for c in clients {
        if !c.mapped || c.hidden {
            continue;
        }
        let on_active_ws = if is_special_active {
            c.workspace.id == active_workspace_id
                || c.workspace.id < 0
                || c.workspace.name.starts_with("special")
        } else {
            c.workspace.id == active_workspace_id
        };
        if !c.pinned && !on_active_ws {
            continue;
        }
        if c.at.len() < 2 || c.size.len() < 2 {
            continue;
        }
        let wx = c.at[0] as f32;
        let wy = c.at[1] as f32;
        let ww = c.size[0] as f32;
        let wh = c.size[1] as f32;

        if raw_cursor.x >= wx && raw_cursor.x < wx + ww && raw_cursor.y >= wy && raw_cursor.y < wy + wh {
            let hw = HyprWindow {
                address: c.address.clone(),
                class: c.class.clone(),
                initial_class: c.initial_class.clone(),
                title: c.title.clone(),
                pid: c.pid,
                workspace: c.workspace.clone(),
                at: c.at.clone(),
                size: c.size.clone(),
                pinned: c.pinned,
                floating: c.floating,
                mapped: c.mapped,
                hidden: c.hidden,
            };
            let c_is_pip = is_pip(&c.class, &c.title);
            if window_under_cursor.is_none()
                || c.floating
                || (window_under_cursor.as_ref().map(|w| is_pip(&w.class, &w.title)).unwrap_or(false) && !c_is_pip)
            {
                window_under_cursor = Some(hw);
            }
        }
    }

    if let Some(w_under) = window_under_cursor {
        w_under
    } else {
        // Cursor is NOT directly over any client on the active workspace (i.e. on empty desktop/workspace).
        // If active_window is PiP or on another workspace, reset to default (empty).
        let active_is_pip = is_pip(&active_window.class, &active_window.title);
        let active_is_on_ws = active_window.pinned
            || (if is_special_active {
                active_window.workspace.id == active_workspace_id
                    || active_window.workspace.id < 0
                    || active_window.workspace.name.starts_with("special")
            } else {
                active_window.workspace.id != 0 && active_window.workspace.id == active_workspace_id
            });

        if active_is_pip || !active_is_on_ws {
            HyprWindow::default()
        } else {
            // Check if active_window bounding box actually covers the cursor
            let covers_cursor = if active_window.at.len() >= 2 && active_window.size.len() >= 2 {
                let wx = active_window.at[0] as f32;
                let wy = active_window.at[1] as f32;
                let ww = active_window.size[0] as f32;
                let wh = active_window.size[1] as f32;
                raw_cursor.x >= wx && raw_cursor.x < wx + ww && raw_cursor.y >= wy && raw_cursor.y < wy + wh
            } else {
                false
            };

            if covers_cursor {
                active_window.clone()
            } else {
                // User opened dial on empty space of the current workspace
                HyprWindow::default()
            }
        }
    }
}

/// Fetch cursor pos, active window, clients, and monitors concurrently via tokio::join!
pub async fn get_context() -> anyhow::Result<HyprContext> {
    if get_hypr_socket().is_none() {
        return Err(anyhow::anyhow!("Hyprland socket not found"));
    }

    let (cursor_res, window_res, clients_res, monitors_res) = tokio::join!(
        query("j/cursorpos"),
        query("j/activewindow"),
        query("j/clients"),
        query("j/monitors"),
    );

    // If all failed, return the first error encountered
    if cursor_res.is_err() && window_res.is_err() && clients_res.is_err() && monitors_res.is_err() {
        return Err(cursor_res.unwrap_err());
    }

    let raw_cursor: HyprCursorPos = cursor_res
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

    let monitors: Vec<HyprMonitor> = monitors_res
        .ok()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    let (target_mon, local_cursor) = resolve_target_monitor_and_local_cursor(&raw_cursor, &monitors);
    let target_monitor_name = target_mon.map(|m| m.name.clone());

    let is_special_on_mon = target_mon
        .as_ref()
        .map(|m| m.special_workspace.id != 0 && !m.special_workspace.name.is_empty())
        .unwrap_or(false);

    let is_special_active = is_special_on_mon
        || window.workspace.id < 0
        || window.workspace.name.starts_with("special");

    let active_ws_id = if is_special_on_mon {
        target_mon.as_ref().unwrap().special_workspace.id
    } else if window.workspace.id < 0 {
        window.workspace.id
    } else if let Some(m) = target_mon {
        m.active_workspace.id
    } else {
        window.workspace.id
    };

    let effective_window = resolve_effective_window(
        &raw_cursor,
        &window,
        &clients,
        active_ws_id,
        is_special_active,
    );

    let is_special_workspace = is_special_active
        || effective_window.workspace.id < 0
        || effective_window.workspace.name.starts_with("special");

    Ok(HyprContext {
        cursor: local_cursor,
        window: effective_window,
        clients,
        monitors,
        target_monitor_name,
        is_special_workspace,
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
                                r#"[{"address": "0x42", "class": "kitty", "title": "myterm", "pid": 999, "workspace": {"id": 1, "name": "1"}, "at": [0, 0], "size": [1920, 1080], "mapped": true, "hidden": false}]"#
                            } else if req.contains("monitors") {
                                r#"[{"id": 0, "name": "eDP-1", "x": 0, "y": 0, "width": 1920, "height": 1080, "scale": 1.0, "transform": 0, "focused": true, "activeWorkspace": {"id": 1, "name": "1"}}]"#
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

    #[test]
    fn test_multi_monitor_cursor_translation() {
        let monitors = vec![
            HyprMonitor {
                id: 0,
                name: "eDP-1".into(),
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
                scale: 1.0,
                transform: 0,
                focused: false,
                active_workspace: HyprWorkspace { id: 1, name: "1".into() },
                special_workspace: HyprWorkspace::default(),
            },
            HyprMonitor {
                id: 1,
                name: "DP-1".into(),
                x: 1920,
                y: 0,
                width: 2560,
                height: 1440,
                scale: 1.25,
                transform: 0,
                focused: true,
                active_workspace: HyprWorkspace { id: 2, name: "2".into() },
                special_workspace: HyprWorkspace::default(),
            },
        ];

        // Cursor is on second monitor at global coordinates (2500, 500)
        let raw_cursor = HyprCursorPos { x: 2500.0, y: 500.0 };
        let (mon, local) = resolve_target_monitor_and_local_cursor(&raw_cursor, &monitors);
        assert_eq!(mon.unwrap().name, "DP-1");
        assert_eq!(local.x, 2500.0 - 1920.0);
        assert_eq!(local.y, 500.0 - 0.0);
    }

    #[test]
    fn test_empty_workspace_with_pip_resolves_default() {
        let clients = vec![
            HyprClient {
                address: "0xpip".into(),
                class: "firefox".into(),
                initial_class: "firefox".into(),
                title: "Picture-in-Picture".into(),
                pid: 1234,
                workspace: HyprWorkspace { id: 1, name: "1".into() },
                at: vec![1200, 700],
                size: vec![640, 360],
                floating: true,
                pinned: true,
                mapped: true,
                hidden: false,
            }
        ];
        let active_win = HyprWindow {
            address: "0xpip".into(),
            class: "firefox".into(),
            initial_class: "firefox".into(),
            title: "Picture-in-Picture".into(),
            pid: 1234,
            workspace: HyprWorkspace { id: 1, name: "1".into() },
            at: vec![1200, 700],
            size: vec![640, 360],
            floating: true,
            pinned: true,
            mapped: true,
            hidden: false,
        };
        // Cursor on empty workspace 3 at (500, 400), away from the PiP window (at 1200, 700)
        let raw_cursor = HyprCursorPos { x: 500.0, y: 400.0 };
        let active_ws_id = 3;
        let eff = resolve_effective_window(&raw_cursor, &active_win, &clients, active_ws_id, false);
        assert_eq!(eff.class, "");
        assert_eq!(eff.title, "");
        assert_eq!(crate::state::context::resolve_context(&eff.class, &eff.title), crate::state::context::Context::Default);
    }

    #[test]
    fn test_special_workspace_effective_window() {
        let clients = vec![
            HyprClient {
                address: "0xspecial_kitty".into(),
                class: "kitty".into(),
                initial_class: "kitty".into(),
                title: "agy".into(),
                pid: 5678,
                workspace: HyprWorkspace { id: -99, name: "special:special".into() },
                at: vec![100, 100],
                size: vec![800, 600],
                floating: false,
                pinned: false,
                mapped: true,
                hidden: false,
            }
        ];
        let active_win = HyprWindow {
            address: "0xspecial_kitty".into(),
            class: "kitty".into(),
            initial_class: "kitty".into(),
            title: "agy".into(),
            pid: 5678,
            workspace: HyprWorkspace { id: -99, name: "special:special".into() },
            at: vec![100, 100],
            size: vec![800, 600],
            floating: false,
            pinned: false,
            mapped: true,
            hidden: false,
        };

        // Cursor inside the kitty window at (200, 200)
        let raw_cursor = HyprCursorPos { x: 200.0, y: 200.0 };
        let active_ws_id = -99;
        let eff = resolve_effective_window(&raw_cursor, &active_win, &clients, active_ws_id, true);
        assert_eq!(eff.class, "kitty");
        assert_eq!(eff.workspace.name, "special:special");
        assert_eq!(eff.workspace.id, -99);
    }
}
