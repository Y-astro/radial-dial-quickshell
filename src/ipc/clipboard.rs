//! Clipboard history querying and pasting via `cliphist`, `wl-copy`, and `wtype`.
//!
//! Provides asynchronous utilities for:
//! - Fetching recent clipboard history entries (`get_recent_clips`)
//! - Parsing `cliphist list` output (`parse_cliphist_output`)
//! - Decoding and pasting an entry by ID (`paste_clip_item`)

use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ClipItem {
    pub id: String,
    pub preview: String,
    pub full: String,
}

/// Parse the raw stdout from `cliphist list`.
/// Each line in `cliphist list` is formatted as `<id>\t<text>`.
/// Previews are truncated to 36 characters with trailing ellipsis.
pub fn parse_cliphist_output(stdout: &str, limit: usize) -> Vec<ClipItem> {
    let mut items = Vec::new();
    for line in stdout.lines() {
        if items.len() >= limit {
            break;
        }
        let parts: Vec<&str> = line.splitn(2, '\t').collect();
        if parts.len() == 2 {
            let id = parts[0].trim().to_string();
            let full = parts[1].trim().to_string();
            let char_count = full.chars().count();
            let preview = if char_count > 36 {
                let prefix: String = full.chars().take(33).collect();
                format!("{}...", prefix)
            } else {
                full.clone()
            };
            items.push(ClipItem { id, preview, full });
        }
    }
    items
}

/// Query recent clipboard snippets from cliphist (up to `limit` items).
/// Port of `cmd_clipboard` in `hypr_ipc.py`.
pub async fn get_recent_clips(limit: usize) -> Vec<ClipItem> {
    let output = match Command::new("cliphist").arg("list").output().await {
        Ok(out) if out.status.success() => out,
        Ok(out) => {
            log::warn!("cliphist list exited with status: {:?}", out.status);
            return Vec::new();
        }
        Err(e) => {
            log::warn!("Failed to execute cliphist list: {}", e);
            return Vec::new();
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_cliphist_output(&stdout, limit)
}

/// Decode and paste clipboard item by ID.
/// Port of `cmd_paste` in `hypr_ipc.py`:
/// Runs `cliphist decode {id} | wl-copy`, pauses briefly, then triggers `wtype -M ctrl -k v -m ctrl`.
pub async fn paste_clip_item(id: &str) -> anyhow::Result<()> {
    let decode_output = Command::new("cliphist")
        .args(["decode", id])
        .output()
        .await?;

    if !decode_output.status.success() {
        anyhow::bail!(
            "cliphist decode failed for id {}: {}",
            id,
            String::from_utf8_lossy(&decode_output.stderr)
        );
    }

    let mut wl_copy = Command::new("wl-copy")
        .stdin(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = wl_copy.stdin.take() {
        stdin.write_all(&decode_output.stdout).await?;
        drop(stdin);
    }

    let copy_status = wl_copy.wait().await?;
    if !copy_status.success() {
        anyhow::bail!("wl-copy exited with error: {:?}", copy_status);
    }

    // Brief pause to allow target window to regain focus after overlay surface unmaps
    tokio::time::sleep(tokio::time::Duration::from_millis(80)).await;

    let wtype_status = Command::new("wtype")
        .args(["-M", "ctrl", "-k", "v", "-m", "ctrl"])
        .status()
        .await?;

    if !wtype_status.success() {
        anyhow::bail!("wtype exited with error: {:?}", wtype_status);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cliphist_output() {
        let mock_stdout = "\
1\tShort line
2\tThis is a very long line that should definitely exceed thirty-six characters in total length
3\tThird item
4\tFourth item
5\tFifth item
";

        let clips = parse_cliphist_output(mock_stdout, 8);
        assert_eq!(clips.len(), 5);

        // First item (<= 36 chars)
        assert_eq!(clips[0].id, "1");
        assert_eq!(clips[0].preview, "Short line");
        assert_eq!(clips[0].full, "Short line");

        // Second item (> 36 chars, truncated to 33 chars + "...")
        assert_eq!(clips[1].id, "2");
        assert_eq!(clips[1].preview, "This is a very long line that sho...");
        assert_eq!(clips[1].preview.chars().count(), 36);
        assert_eq!(
            clips[1].full,
            "This is a very long line that should definitely exceed thirty-six characters in total length"
        );

        // Third item
        assert_eq!(clips[2].id, "3");
        assert_eq!(clips[2].preview, "Third item");
    }

    #[test]
    fn test_parse_cliphist_output_limit() {
        let mock_stdout = "\
10\tItem 1
20\tItem 2
30\tItem 3
40\tItem 4
50\tItem 5
";
        let clips = parse_cliphist_output(mock_stdout, 3);
        assert_eq!(clips.len(), 3);
        assert_eq!(clips[0].id, "10");
        assert_eq!(clips[1].id, "20");
        assert_eq!(clips[2].id, "30");
    }

    #[test]
    fn test_parse_cliphist_output_malformed_lines() {
        let mock_stdout = "\
no_tab_here
\tleading_tab_only
100\tvalid item
another_invalid_line
200\tsecond valid
";
        let clips = parse_cliphist_output(mock_stdout, 10);
        assert_eq!(clips.len(), 3);
        assert_eq!(clips[0].id, "");
        assert_eq!(clips[0].preview, "leading_tab_only");
        assert_eq!(clips[1].id, "100");
        assert_eq!(clips[1].preview, "valid item");
        assert_eq!(clips[2].id, "200");
        assert_eq!(clips[2].preview, "second valid");
    }

    #[test]
    fn test_parse_cliphist_utf8_truncation() {
        let mock_stdout = "1\t🦀 Rust Wayland 🚀 高性能 Radial Dial 🎨 菜单界面测试超出字符限制长度\n";
        let clips = parse_cliphist_output(mock_stdout, 8);
        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0].preview.chars().count(), 36);
        assert!(clips[0].preview.ends_with("..."));
    }

    #[test]
    fn test_clip_item_serde_roundtrip() {
        let item = ClipItem {
            id: "42".into(),
            preview: "Hello...".into(),
            full: "Hello world".into(),
        };
        let json = serde_json::to_string(&item).unwrap();
        let deserialized: ClipItem = serde_json::from_str(&json).unwrap();
        assert_eq!(item, deserialized);
    }

    #[tokio::test]
    async fn test_live_get_recent_clips() {
        let clips = get_recent_clips(5).await;
        assert!(clips.len() <= 5);
    }
}
