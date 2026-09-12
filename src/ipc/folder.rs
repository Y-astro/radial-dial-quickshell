//! Filesystem navigation and block device mounting IPC for the folder browser modal.
//!
//! Provides utilities for:
//! - Listing directory subfolders with breadcrumb hierarchy (`list_dir`, `list_dir_sync`)
//! - Generating breadcrumb trails from root to target path (`compute_breadcrumbs`)
//! - Listing system places and detected storage drives (`get_places`, `parse_lsblk_json`)
//! - Mounting external block devices via `udisksctl` and `lsblk` (`mount_device`)

use std::path::{Path, PathBuf};
use tokio::process::Command;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Breadcrumb {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FolderEntry {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DirListing {
    pub path: String,
    pub parent: String,
    pub name: String,
    pub crumbs: Vec<Breadcrumb>,
    pub folders: Vec<FolderEntry>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PlaceEntry {
    pub name: String,
    pub path: String,
    pub icon: String,
    #[serde(default)]
    pub is_device: bool,
    #[serde(default, alias = "dev")]
    pub dev_node: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct LsblkResponse {
    #[serde(default)]
    blockdevices: Vec<LsblkDeviceNode>,
}

#[derive(Debug, serde::Deserialize)]
struct LsblkDeviceNode {
    name: String,
    label: Option<String>,
    #[serde(default)]
    mountpoints: Vec<Option<String>>,
    #[serde(default)]
    children: Vec<LsblkDeviceNode>,
}

/// Computes the breadcrumb trail from `Root (/)` to the target path.
/// Matches the hierarchy format in `folder_browser.py`.
pub fn compute_breadcrumbs(path: &Path) -> Vec<Breadcrumb> {
    let mut crumbs = Vec::new();
    let mut curr = Some(path);

    while let Some(p) = curr {
        if p == Path::new("/") || p.as_os_str().is_empty() {
            break;
        }
        let name = p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        let path_str = p.to_string_lossy().to_string();
        if !name.is_empty() {
            crumbs.push(Breadcrumb {
                name,
                path: path_str,
            });
        }
        curr = p.parent();
    }

    crumbs.reverse();
    crumbs.insert(
        0,
        Breadcrumb {
            name: "Root (/)".to_string(),
            path: "/".to_string(),
        },
    );
    crumbs
}

/// Parses mount path from `udisksctl mount` stdout and stderr output.
pub fn parse_mount_output(stdout: &str, stderr: &str) -> Option<String> {
    // 1. Success message in stdout: "Mounted /dev/sdb1 at /run/media/username/LABEL."
    for line in stdout.lines() {
        if let Some(pos) = line.find(" at ") {
            let path_part = line[pos + 4..].trim();
            let path = path_part.trim_end_matches('.');
            if !path.is_empty() {
                return Some(path.to_string());
            }
        }
    }

    // 2. Already mounted message in stderr:
    // "Device /dev/sdb1 is already mounted at '/run/media/username/LABEL'."
    // or "... already mounted at `/run/media/username/LABEL'"
    for line in stderr.lines() {
        if let Some(pos) = line.find("already mounted at ") {
            let rest = line[pos + "already mounted at ".len()..].trim();
            let trimmed = rest.trim_matches(|c: char| c == '`' || c == '\'' || c == '"' || c == '.');
            if let Some(end_idx) = trimmed.find(|c: char| c == '\'' || c == '`' || c == '"') {
                return Some(trimmed[..end_idx].to_string());
            } else if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }

    None
}

/// Parses `lsblk -J` JSON structure to extract eligible storage devices.
/// Filters out internal swap, boot, and system root partitions.
pub fn parse_lsblk_json(json_str: &str) -> Vec<PlaceEntry> {
    let Ok(data) = serde_json::from_str::<LsblkResponse>(json_str) else {
        return Vec::new();
    };

    let ignored_labels = [
        "SYSTEM", "linux-boot", "linux-swap", "zram0", "RESTORE", "MYASUS", "linux-root",
    ];
    let ignored_mountpoints = ["/", "/home", "/boot/efi", "/boot", "/var/tmp"];

    fn scan_node(
        node: &LsblkDeviceNode,
        ignored_labels: &[&str],
        ignored_mountpoints: &[&str],
        out: &mut Vec<PlaceEntry>,
    ) {
        if let Some(label) = &node.label {
            if !label.is_empty() && !ignored_labels.contains(&label.as_str()) {
                let mp = node
                    .mountpoints
                    .iter()
                    .filter_map(|m| m.as_deref())
                    .find(|m| !m.is_empty() && !m.starts_with('['))
                    .unwrap_or("");

                if !ignored_mountpoints.contains(&mp) {
                    let dev_path = format!("/dev/{}", node.name);
                    let target_path = if !mp.is_empty() {
                        mp.to_string()
                    } else {
                        dev_path.clone()
                    };
                    out.push(PlaceEntry {
                        name: label.clone(),
                        path: target_path,
                        icon: "hard_drive".to_string(),
                        is_device: true,
                        dev_node: Some(dev_path),
                    });
                }
            }
        }

        for child in &node.children {
            scan_node(child, ignored_labels, ignored_mountpoints, out);
        }
    }

    let mut places = Vec::new();
    for dev in &data.blockdevices {
        scan_node(dev, &ignored_labels, &ignored_mountpoints, &mut places);
    }
    places
}

/// Synchronously lists subdirectories of `target_path` and computes breadcrumb hierarchy.
/// Ignores hidden folders (starting with '.'), `$RECYCLE.BIN`, and `System Volume Information`.
pub fn list_dir_sync(target_path: &str) -> DirListing {
    let raw = target_path.trim();
    let raw = if raw.is_empty() { "~" } else { raw };
    let expanded = shellexpand::tilde(raw).to_string();
    let mut target = PathBuf::from(expanded);

    if !target.is_dir() {
        if let Ok(home) = std::env::var("HOME") {
            let home_path = PathBuf::from(home);
            if home_path.is_dir() {
                target = home_path;
            } else {
                target = PathBuf::from("/");
            }
        } else {
            target = PathBuf::from("/");
        }
    }

    let abs_path = std::fs::canonicalize(&target).unwrap_or(target);
    let path_str = abs_path.to_string_lossy().to_string();
    let parent = abs_path
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "/".to_string());
    let mut name = abs_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("/")
        .to_string();
    if name.is_empty() {
        name = "/".to_string();
    }

    let crumbs = compute_breadcrumbs(&abs_path);

    let mut folders = Vec::new();
    match std::fs::read_dir(&abs_path) {
        Ok(read_dir) => {
            for entry in read_dir.flatten() {
                let fname = entry.file_name().to_string_lossy().to_string();
                if fname.starts_with('.')
                    || fname == "$RECYCLE.BIN"
                    || fname == "System Volume Information"
                {
                    continue;
                }
                let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false)
                    || entry.path().is_dir();
                if is_dir {
                    folders.push(FolderEntry {
                        name: fname,
                        path: entry.path().to_string_lossy().to_string(),
                    });
                }
            }
            folders.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            DirListing {
                path: path_str,
                parent,
                name,
                crumbs,
                folders,
                error: None,
            }
        }
        Err(e) => DirListing {
            path: path_str,
            parent,
            name,
            crumbs,
            folders: Vec::new(),
            error: Some(e.to_string()),
        },
    }
}

/// List directory subfolders with breadcrumb hierarchy. Ignores hidden folders and $RECYCLE.BIN.
/// If target is a block device (`/dev/...`), mounts it first.
pub async fn list_dir(target_path: &str) -> DirListing {
    let mut resolved = target_path.trim().to_string();
    if resolved.is_empty() {
        resolved = "~".to_string();
    }

    // Auto-mount if pointing to a block device
    if resolved.starts_with("/dev/") {
        if let Ok(mounted) = mount_device(&resolved).await {
            resolved = mounted;
        }
    }

    tokio::task::spawn_blocking(move || list_dir_sync(&resolved))
        .await
        .unwrap_or_else(|e| DirListing {
            path: target_path.to_string(),
            parent: "/".to_string(),
            name: target_path.to_string(),
            crumbs: Vec::new(),
            folders: Vec::new(),
            error: Some(e.to_string()),
        })
}

/// List system places (Home, Documents, Downloads, Pictures, Music) + block devices from `lsblk -J`.
pub async fn get_places() -> Vec<PlaceEntry> {
    let mut places = Vec::new();
    let home = std::env::var("HOME")
        .unwrap_or_else(|_| shellexpand::tilde("~").to_string());

    let candidates = [
        ("Home", "home", home.clone()),
        ("Documents", "description", format!("{}/Documents", home)),
        ("Downloads", "download", format!("{}/Downloads", home)),
        ("Pictures", "photo", format!("{}/Pictures", home)),
        ("Music", "music_note", format!("{}/Music", home)),
    ];

    for (name, icon, path) in candidates {
        if Path::new(&path).is_dir() {
            places.push(PlaceEntry {
                name: name.to_string(),
                path,
                icon: icon.to_string(),
                is_device: false,
                dev_node: None,
            });
        }
    }

    // Block devices via lsblk -J
    let lsblk_res = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        Command::new("lsblk")
            .args(["-J", "-o", "NAME,LABEL,MOUNTPOINTS,FSTYPE,SIZE"])
            .output(),
    )
    .await;

    if let Ok(Ok(output)) = lsblk_res {
        if output.status.success() {
            let json_str = String::from_utf8_lossy(&output.stdout);
            let mut devices = parse_lsblk_json(&json_str);
            places.append(&mut devices);
        }
    }

    places.push(PlaceEntry {
        name: "Root (/)".to_string(),
        path: "/".to_string(),
        icon: "computer".to_string(),
        is_device: false,
        dev_node: None,
    });

    places
}

/// Mount block device using `udisksctl mount -b {device_node}`. Returns mounted directory path.
pub async fn mount_device(dev_node: &str) -> anyhow::Result<String> {
    let node = if dev_node.starts_with("/dev/") {
        dev_node.to_string()
    } else {
        format!("/dev/{}", dev_node)
    };

    // 1. Try udisksctl mount
    let udisks_res = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        Command::new("udisksctl")
            .args(["mount", "-b", &node])
            .output(),
    )
    .await;

    if let Ok(Ok(output)) = udisks_res {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        if let Some(path) = parse_mount_output(&stdout, &stderr) {
            return Ok(path);
        }
    }

    // 2. Query lsblk mountpoint fallback
    let lsblk_res = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        Command::new("lsblk")
            .args(["-no", "MOUNTPOINT", &node])
            .output(),
    )
    .await;

    if let Ok(Ok(output)) = lsblk_res {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() && !trimmed.starts_with('[') {
                return Ok(trimmed.to_string());
            }
        }
    }

    if Path::new(&node).exists() {
        Ok(node)
    } else {
        anyhow::bail!("Failed to mount device node {}", node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dir_listing_current_dir() {
        let listing = list_dir(".").await;
        assert!(listing.error.is_none(), "Listing current directory should not error: {:?}", listing.error);
        assert!(!listing.path.is_empty());
        assert!(!listing.name.is_empty());
        assert!(!listing.crumbs.is_empty());
        assert_eq!(listing.crumbs[0].name, "Root (/)");
        assert_eq!(listing.crumbs[0].path, "/");

        // Verify subfolders list contains 'src'
        let has_src = listing.folders.iter().any(|f| f.name == "src");
        assert!(has_src, "Current directory listing should include 'src'");

        // Verify hidden folders and .superpowers are excluded
        for f in &listing.folders {
            assert!(!f.name.starts_with('.'), "Hidden folder '{}' should not be present", f.name);
            assert_ne!(f.name, "$RECYCLE.BIN");
            assert_ne!(f.name, "System Volume Information");
        }
    }

    #[test]
    fn test_breadcrumbs_generation() {
        // Root path
        let root_crumbs = compute_breadcrumbs(Path::new("/"));
        assert_eq!(root_crumbs.len(), 1);
        assert_eq!(root_crumbs[0].name, "Root (/)");
        assert_eq!(root_crumbs[0].path, "/");

        // Single level
        let usr_crumbs = compute_breadcrumbs(Path::new("/usr"));
        assert_eq!(usr_crumbs.len(), 2);
        assert_eq!(usr_crumbs[0].name, "Root (/)");
        assert_eq!(usr_crumbs[0].path, "/");
        assert_eq!(usr_crumbs[1].name, "usr");
        assert_eq!(usr_crumbs[1].path, "/usr");

        // Deeply nested
        let nested = Path::new("/home/astro/repos/radial-dial-rust");
        let crumbs = compute_breadcrumbs(nested);
        assert_eq!(crumbs.len(), 5);
        assert_eq!(crumbs[0].name, "Root (/)");
        assert_eq!(crumbs[0].path, "/");
        assert_eq!(crumbs[1].name, "home");
        assert_eq!(crumbs[1].path, "/home");
        assert_eq!(crumbs[2].name, "astro");
        assert_eq!(crumbs[2].path, "/home/astro");
        assert_eq!(crumbs[3].name, "repos");
        assert_eq!(crumbs[3].path, "/home/astro/repos");
        assert_eq!(crumbs[4].name, "radial-dial-rust");
        assert_eq!(crumbs[4].path, "/home/astro/repos/radial-dial-rust");
    }

    #[test]
    fn test_parse_mount_output() {
        // Standard stdout
        let stdout = "Mounted /dev/sdb1 at /run/media/astro/USB_DRIVE.\n";
        assert_eq!(
            parse_mount_output(stdout, ""),
            Some("/run/media/astro/USB_DRIVE".to_string())
        );

        // Stderr single quotes
        let stderr = "Error mounting /dev/sdb1: Device /dev/sdb1 is already mounted at '/run/media/astro/MY_DISK'.\n";
        assert_eq!(
            parse_mount_output("", stderr),
            Some("/run/media/astro/MY_DISK".to_string())
        );

        // Stderr backticks
        let stderr_bt = "Error mounting /dev/sdc1: already mounted at `/media/astro/EXTERNAL'\n";
        assert_eq!(
            parse_mount_output("", stderr_bt),
            Some("/media/astro/EXTERNAL".to_string())
        );

        // None on unrelated output
        assert_eq!(parse_mount_output("Nothing here\n", "No error\n"), None);
    }

    #[test]
    fn test_parse_lsblk_json() {
        let json_sample = r#"{
           "blockdevices": [
              {
                 "name": "zram0",
                 "label": "zram0",
                 "mountpoints": ["[SWAP]"]
              },
              {
                 "name": "nvme0n1",
                 "label": null,
                 "mountpoints": [],
                 "children": [
                    {
                       "name": "nvme0n1p1",
                       "label": "SYSTEM",
                       "mountpoints": []
                    },
                    {
                       "name": "nvme0n1p5",
                       "label": "linux-boot",
                       "mountpoints": ["/boot/efi"]
                    },
                    {
                       "name": "nvme0n1p6",
                       "label": "linux-root",
                       "mountpoints": ["/"]
                    },
                    {
                       "name": "nvme0n1p9",
                       "label": "External Data",
                       "mountpoints": ["/run/media/astro/External Data"]
                    },
                    {
                       "name": "sdb1",
                       "label": "USB Stick",
                       "mountpoints": []
                    }
                 ]
              }
           ]
        }"#;

        let places = parse_lsblk_json(json_sample);
        assert_eq!(places.len(), 2);

        assert_eq!(places[0].name, "External Data");
        assert_eq!(places[0].path, "/run/media/astro/External Data");
        assert_eq!(places[0].icon, "hard_drive");
        assert!(places[0].is_device);
        assert_eq!(places[0].dev_node.as_deref(), Some("/dev/nvme0n1p9"));

        assert_eq!(places[1].name, "USB Stick");
        assert_eq!(places[1].path, "/dev/sdb1");
        assert_eq!(places[1].icon, "hard_drive");
        assert!(places[1].is_device);
        assert_eq!(places[1].dev_node.as_deref(), Some("/dev/sdb1"));
    }

    #[tokio::test]
    async fn test_get_places() {
        let places = get_places().await;
        assert!(!places.is_empty(), "get_places should return at least Root and Home if available");
        let has_root = places.iter().any(|p| p.path == "/" && p.name == "Root (/)");
        assert!(has_root, "get_places must contain Root (/)");
    }

    #[test]
    fn test_list_dir_ignores_hidden_and_recycle_bin() {
        let tmp = std::env::temp_dir().join(format!("radial_dial_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("create tempdir");

        // Create various directories and files
        std::fs::create_dir(tmp.join("VisibleFolder")).unwrap();
        std::fs::create_dir(tmp.join("another_folder")).unwrap();
        std::fs::create_dir(tmp.join(".hidden_folder")).unwrap();
        std::fs::create_dir(tmp.join("$RECYCLE.BIN")).unwrap();
        std::fs::create_dir(tmp.join("System Volume Information")).unwrap();
        std::fs::write(tmp.join("test_file.txt"), "hello").unwrap();

        let listing = list_dir_sync(tmp.to_str().unwrap());
        assert!(listing.error.is_none());
        assert_eq!(listing.folders.len(), 2);
        assert_eq!(listing.folders[0].name, "another_folder");
        assert_eq!(listing.folders[1].name, "VisibleFolder");

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
