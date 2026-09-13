//! GPU Profile detection from config override, sysfs, or lspci.
//!
//! Determines whether the system should run in `LowEnd` or `HighPerformance`
//! mode for rendering, animations, and visual effects.

use std::path::Path;
use crate::state::config::RadialConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum GpuProfile {
    LowEnd,
    HighPerformance,
}

/// Detect GPU profile:
/// 1. Check user config override (performance.profile)
/// 2. Scan /sys/class/drm/card*/device/vendor for 0x10de (NVIDIA)
/// 3. Fallback: run `lspci -nn` and match nvidia|geforce|quadro|rtx|arc|radeon rx|radeon pro
///
/// Defaults to `LowEnd` if no discrete GPU detected.
pub fn detect_gpu_profile(config: &RadialConfig) -> GpuProfile {
    detect_gpu_profile_internal(
        config,
        Path::new("/sys/class/drm"),
        check_lspci_discrete_gpu,
    )
}

/// Internal helper allowing injection of sysfs path and lspci check for deterministic testing.
pub fn detect_gpu_profile_internal(
    config: &RadialConfig,
    sysfs_drm_dir: &Path,
    lspci_check: impl Fn() -> bool,
) -> GpuProfile {
    // 1. Check user config override (performance.profile)
    if let Some(perf) = &config.performance {
        if let Some(profile_str) = &perf.profile {
            match profile_str.trim().to_lowercase().as_str() {
                "low" | "low_end" | "low-end" => return GpuProfile::LowEnd,
                "high" | "high_performance" | "high-performance" => {
                    return GpuProfile::HighPerformance
                }
                _ => {} // "auto" or unrecognized: proceed to hardware detection
            }
        }
    }

    // 2. Scan sysfs for NVIDIA vendor ID (0x10de)
    if check_sysfs_nvidia(sysfs_drm_dir) {
        return GpuProfile::HighPerformance;
    }

    // 3. Fallback: run lspci -nn and match discrete GPU patterns
    if lspci_check() {
        return GpuProfile::HighPerformance;
    }

    // Defaults to LowEnd if no discrete GPU detected
    GpuProfile::LowEnd
}

/// Scan `/sys/class/drm/card*/device/vendor` for `0x10de` (NVIDIA).
pub fn check_sysfs_nvidia(drm_dir: &Path) -> bool {
    let entries = match std::fs::read_dir(drm_dir) {
        Ok(e) => e,
        Err(_) => return false,
    };

    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let name_str = file_name.to_string_lossy();
        // Match cardN directories, excluding connector subdirectories like card1-HDMI-A-1
        if name_str.starts_with("card") && !name_str.contains('-') {
            let vendor_path = entry.path().join("device").join("vendor");
            if let Ok(vendor) = std::fs::read_to_string(vendor_path) {
                if vendor.trim().to_lowercase().contains("0x10de") {
                    return true;
                }
            }
        }
    }

    false
}

/// Parse `lspci -nn` stdout lines to detect discrete GPUs.
/// Matches nvidia|geforce|quadro|rtx|arc|radeon rx|radeon pro on display controller lines.
pub fn parse_lspci_discrete_gpu(stdout: &str) -> bool {
    for line in stdout.lines() {
        let low = line.to_lowercase();
        let is_display_controller = low.contains("vga compatible controller")
            || low.contains("3d controller")
            || low.contains("display controller");

        let matches_discrete_pattern = low.contains("nvidia")
            || low.contains("geforce")
            || low.contains("quadro")
            || low.contains("rtx")
            || low.contains("arc")
            || low.contains("radeon rx")
            || low.contains("radeon pro");

        if is_display_controller && matches_discrete_pattern {
            return true;
        }
    }
    false
}

/// Run `lspci -nn` via subprocess and check for discrete GPU presence.
pub fn check_lspci_discrete_gpu() -> bool {
    match std::process::Command::new("lspci").arg("-nn").output() {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            parse_lspci_discrete_gpu(&stdout)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::config::{PerformanceConfig, RadialConfig};

    fn make_test_config(profile: Option<&str>) -> RadialConfig {
        RadialConfig {
            global_slices: vec![],
            kitty_slices: vec![],
            browser_slices: vec![],
            code_slices: vec![],
            media_slices: vec![],
            file_jump_targets: vec![],
            performance: Some(PerformanceConfig {
                profile: profile.map(String::from),
            }),
            custom_colors: None,
            colors: None,
            has_explicit_colors: false,
        }
    }

    #[test]
    fn test_gpu_profile_config_override() {
        let empty_path = Path::new("/nonexistent_sysfs_path_12345");
        let no_lspci = || false;

        // "low_end" override
        let cfg = make_test_config(Some("low_end"));
        assert_eq!(
            detect_gpu_profile_internal(&cfg, empty_path, no_lspci),
            GpuProfile::LowEnd
        );

        // "low" override
        let cfg = make_test_config(Some("low"));
        assert_eq!(
            detect_gpu_profile_internal(&cfg, empty_path, no_lspci),
            GpuProfile::LowEnd
        );

        // "low-end" override
        let cfg = make_test_config(Some("low-end"));
        assert_eq!(
            detect_gpu_profile_internal(&cfg, empty_path, no_lspci),
            GpuProfile::LowEnd
        );

        // "high_performance" override
        let cfg = make_test_config(Some("high_performance"));
        assert_eq!(
            detect_gpu_profile_internal(&cfg, empty_path, no_lspci),
            GpuProfile::HighPerformance
        );

        // "high" override
        let cfg = make_test_config(Some("high"));
        assert_eq!(
            detect_gpu_profile_internal(&cfg, empty_path, no_lspci),
            GpuProfile::HighPerformance
        );

        // "high-performance" case insensitivity
        let cfg = make_test_config(Some("HIGH_PERFORMANCE"));
        assert_eq!(
            detect_gpu_profile_internal(&cfg, empty_path, no_lspci),
            GpuProfile::HighPerformance
        );

        // "auto" falls back to hardware detection (which returns LowEnd if no dGPU found)
        let cfg = make_test_config(Some("auto"));
        assert_eq!(
            detect_gpu_profile_internal(&cfg, empty_path, no_lspci),
            GpuProfile::LowEnd
        );

        // "auto" with mock lspci returning true -> HighPerformance
        let cfg = make_test_config(Some("auto"));
        assert_eq!(
            detect_gpu_profile_internal(&cfg, empty_path, || true),
            GpuProfile::HighPerformance
        );
    }

    #[test]
    fn test_check_sysfs_nvidia() {
        let temp_dir = std::env::temp_dir().join(format!("test_sysfs_drm_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);

        // Card 0: Intel (0x8086)
        let card0_dev = temp_dir.join("card0").join("device");
        std::fs::create_dir_all(&card0_dev).unwrap();
        std::fs::write(card0_dev.join("vendor"), "0x8086\n").unwrap();

        // Connector directory: should be ignored
        let conn_dir = temp_dir.join("card0-HDMI-A-1");
        std::fs::create_dir_all(&conn_dir).unwrap();

        assert!(!check_sysfs_nvidia(&temp_dir));

        // Card 1: NVIDIA (0x10de)
        let card1_dev = temp_dir.join("card1").join("device");
        std::fs::create_dir_all(&card1_dev).unwrap();
        std::fs::write(card1_dev.join("vendor"), "0x10DE\n").unwrap();

        assert!(check_sysfs_nvidia(&temp_dir));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_parse_lspci_discrete_gpu() {
        // Intel integrated graphics only
        let intel_only = "\
00:02.0 VGA compatible controller [0300]: Intel Corporation UHD Graphics 620 [8086:5917] (rev 07)
00:1f.3 Audio device [0403]: Intel Corporation Sunrise Point-LP HD Audio [8086:9d71] (rev 21)
";
        assert!(!parse_lspci_discrete_gpu(intel_only));

        // NVIDIA RTX discrete GPU
        let nvidia_rtx = "\
00:02.0 Display controller [0380]: Intel Corporation Raptor Lake-S UHD Graphics [8086:a78b] (rev 04)
01:00.0 VGA compatible controller [0300]: NVIDIA Corporation GB206M [GeForce RTX 5060 Max-Q] [10de:2d59] (rev a1)
";
        assert!(parse_lspci_discrete_gpu(nvidia_rtx));

        // AMD Radeon RX
        let amd_rx = "\
03:00.0 VGA compatible controller [0300]: Advanced Micro Devices, Inc. [AMD/ATI] Navi 31 [Radeon RX 7900 XTX] (rev c8)
";
        assert!(parse_lspci_discrete_gpu(amd_rx));

        // Intel Arc
        let intel_arc = "\
03:00.0 VGA compatible controller [0300]: Intel Corporation DG2 [Arc A770] (rev 08)
";
        assert!(parse_lspci_discrete_gpu(intel_arc));

        // Unrelated device with NVIDIA in description (e.g. audio controller, not display controller)
        let audio_nvidia = "\
01:00.1 Audio device [0403]: NVIDIA Corporation High Definition Audio Controller [10de:22ec] (rev a1)
";
        assert!(!parse_lspci_discrete_gpu(audio_nvidia));
    }

    #[test]
    fn test_gpu_profile_serde() {
        let p = GpuProfile::HighPerformance;
        let json = serde_json::to_string(&p).unwrap();
        let deserialized: GpuProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(p, deserialized);
    }

    #[test]
    fn test_live_detect_gpu_profile() {
        let cfg = make_test_config(None);
        let profile = detect_gpu_profile(&cfg);
        // On this host with NVIDIA RTX 5060, it detects HighPerformance
        assert_eq!(profile, GpuProfile::HighPerformance);
    }
}
