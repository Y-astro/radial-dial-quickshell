//! Audio sink management and volume control via PipeWire `wpctl`.
//!
//! Provides asynchronous utilities for:
//! - Querying audio output sinks (`get_audio_sinks`)
//! - Parsing `wpctl status` output (`parse_wpctl_sinks`)
//! - Setting the default audio sink (`set_default_sink`)
//! - Adjusting system volume (`adjust_volume`)

use tokio::process::Command;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AudioSink {
    pub id: String,
    pub name: String,
    #[serde(alias = "fullName")]
    pub full_name: String,
    pub default: bool,
}

/// Parse `wpctl status` stdout and extract the audio Sinks list.
/// Port of `cmd_audio_sinks` in `hypr_ipc.py`.
pub fn parse_wpctl_sinks(stdout: &str) -> Vec<AudioSink> {
    let mut in_sinks = false;
    let mut sinks = Vec::new();

    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.contains("Sinks:") {
            in_sinks = true;
            continue;
        }

        if in_sinks {
            // Stop conditions: next section headers in wpctl tree
            if trimmed.contains("Sources:")
                || trimmed.contains("Filters:")
                || trimmed.contains("Streams:")
                || trimmed.contains("Devices:")
                || trimmed.contains("Video")
                || trimmed.contains("Settings")
            {
                break;
            }

            // Strip tree drawing glyphs and whitespace
            let cleaned = line.trim_start_matches(|c: char| {
                c == '│' || c == '├' || c == '─' || c == '└' || c.is_whitespace()
            });

            if cleaned.is_empty() {
                continue;
            }

            let is_default = cleaned.starts_with('*');
            let rest = if is_default {
                cleaned[1..].trim_start()
            } else {
                cleaned
            };

            // Expect format: <id>.<space><name>...
            if let Some(dot_pos) = rest.find('.') {
                let id_candidate = rest[..dot_pos].trim();
                if !id_candidate.is_empty() && id_candidate.chars().all(|c| c.is_ascii_digit()) {
                    let id = id_candidate.to_string();
                    let after_dot = rest[dot_pos + 1..].trim_start();
                    // Sink name ends before `[` which typically holds volume / mute flags (e.g. [vol: 0.32])
                    let name_part = match after_dot.find('[') {
                        Some(bracket_pos) => &after_dot[..bracket_pos],
                        None => after_dot,
                    };
                    let full_name = name_part.trim().to_string();
                    if !full_name.is_empty() {
                        // Clean up common verbose prefixes
                        let mut clean_name = full_name.clone();
                        for prefix in &["Built-in Audio ", "Family 17h/19h HD Audio Controller "] {
                            clean_name = clean_name.replace(prefix, "");
                        }

                        // Truncate clean_name to 24 characters character-safely
                        let char_count = clean_name.chars().count();
                        let name = if char_count > 24 {
                            clean_name.chars().take(24).collect()
                        } else {
                            clean_name
                        };

                        sinks.push(AudioSink {
                            id,
                            name,
                            full_name,
                            default: is_default,
                        });
                    }
                }
            }
        }
    }

    sinks
}

/// Query audio output sinks from `wpctl status`.
/// Port of `cmd_audio_sinks` in `hypr_ipc.py`.
pub async fn get_audio_sinks() -> Vec<AudioSink> {
    let output = match Command::new("wpctl").arg("status").output().await {
        Ok(out) if out.status.success() => out,
        Ok(out) => {
            log::warn!("wpctl status exited with non-zero status: {:?}", out.status);
            return Vec::new();
        }
        Err(e) => {
            log::warn!("Failed to execute wpctl status: {}", e);
            return Vec::new();
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_wpctl_sinks(&stdout)
}

/// Set default audio output sink: `wpctl set-default {id}`
pub async fn set_default_sink(id: &str) -> anyhow::Result<()> {
    let status = Command::new("wpctl")
        .args(["set-default", id])
        .status()
        .await?;

    if !status.success() {
        anyhow::bail!("wpctl set-default {} exited with status {:?}", id, status);
    }
    Ok(())
}

/// Set volume by delta percent (e.g. +5 or -5) on `@DEFAULT_AUDIO_SINK@`.
pub async fn adjust_volume(delta: i32) -> anyhow::Result<()> {
    if delta == 0 {
        return Ok(());
    }
    let arg = if delta > 0 {
        format!("{}%+", delta)
    } else {
        format!("{}%-", delta.abs())
    };

    let status = Command::new("wpctl")
        .args(["set-volume", "@DEFAULT_AUDIO_SINK@", &arg])
        .status()
        .await?;

    if !status.success() {
        anyhow::bail!("wpctl set-volume {} exited with status {:?}", arg, status);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_wpctl_sinks() {
        let mock_stdout = "\
PipeWire 'pipewire-0' [1.6.8, astro@AstroF16]
Audio
 ├─ Devices:
 │      70. GB206 High Definition Audio Controller [alsa]
 │      71. Built-in Audio                      [alsa]
 │  
 ├─ Sinks:
 │      33. Easy Effects Sink                   [vol: 1.00]
 │  *   82. Built-in Audio Analog Stereo        [vol: 0.32]
 │      95. Family 17h/19h HD Audio Controller Digital Stereo (HDMI 1) [vol: 0.50]
 │  
 ├─ Sources:
 │      34. Easy Effects Source                 [vol: 1.00]
 │  *   83. Built-in Audio Analog Stereo        [vol: 0.30]
";

        let sinks = parse_wpctl_sinks(mock_stdout);
        assert_eq!(sinks.len(), 3);

        // First sink
        assert_eq!(sinks[0].id, "33");
        assert_eq!(sinks[0].name, "Easy Effects Sink");
        assert_eq!(sinks[0].full_name, "Easy Effects Sink");
        assert!(!sinks[0].default);

        // Second sink (default with *)
        assert_eq!(sinks[1].id, "82");
        // "Built-in Audio Analog Stereo" -> prefix "Built-in Audio " removed -> "Analog Stereo"
        assert_eq!(sinks[1].name, "Analog Stereo");
        assert_eq!(sinks[1].full_name, "Built-in Audio Analog Stereo");
        assert!(sinks[1].default);

        // Third sink
        assert_eq!(sinks[2].id, "95");
        // "Family 17h/19h HD Audio Controller Digital Stereo (HDMI 1)" -> prefix removed -> "Digital Stereo (HDMI 1)"
        assert_eq!(sinks[2].name, "Digital Stereo (HDMI 1)");
        assert_eq!(
            sinks[2].full_name,
            "Family 17h/19h HD Audio Controller Digital Stereo (HDMI 1)"
        );
        assert!(!sinks[2].default);
    }

    #[test]
    fn test_parse_wpctl_sinks_empty_or_no_sinks() {
        let mock_stdout = "\
Audio
 ├─ Sinks:
 │  
 ├─ Sources:
 │  *   83. Built-in Audio Analog Stereo
";
        let sinks = parse_wpctl_sinks(mock_stdout);
        assert_eq!(sinks.len(), 0);

        let empty = parse_wpctl_sinks("");
        assert_eq!(empty.len(), 0);
    }

    #[test]
    fn test_parse_wpctl_sinks_name_truncation() {
        let mock_stdout = "\
 ├─ Sinks:
 │      42. Very Long Custom Audio Interface Pro Output Channel 1-2 [vol: 1.00]
 ├─ Sources:
";
        let sinks = parse_wpctl_sinks(mock_stdout);
        assert_eq!(sinks.len(), 1);
        assert_eq!(sinks[0].id, "42");
        assert_eq!(sinks[0].full_name, "Very Long Custom Audio Interface Pro Output Channel 1-2");
        assert_eq!(sinks[0].name, "Very Long Custom Audio I"); // 24 chars
        assert_eq!(sinks[0].name.chars().count(), 24);
    }

    #[test]
    fn test_audio_sink_serde_roundtrip() {
        let sink = AudioSink {
            id: "82".into(),
            name: "Analog Stereo".into(),
            full_name: "Built-in Audio Analog Stereo".into(),
            default: true,
        };
        let json = serde_json::to_string(&sink).unwrap();
        let deserialized: AudioSink = serde_json::from_str(&json).unwrap();
        assert_eq!(sink, deserialized);

        // Test alias "fullName"
        let alias_json = r#"{"id":"82","name":"Analog Stereo","fullName":"Built-in Audio Analog Stereo","default":true}"#;
        let from_alias: AudioSink = serde_json::from_str(alias_json).unwrap();
        assert_eq!(sink, from_alias);
    }

    #[tokio::test]
    async fn test_live_get_audio_sinks() {
        let sinks = get_audio_sinks().await;
        // If wpctl is available and returns sinks on the host, verify fields
        for s in &sinks {
            assert!(!s.id.is_empty());
            assert!(!s.name.is_empty());
            assert!(!s.full_name.is_empty());
        }
    }
}
