#!/usr/bin/env python3
import os, sys, json, subprocess

def mount_dev(dev):
    try:
        p = subprocess.run(["udisksctl", "mount", "-b", dev], capture_output=True, text=True, timeout=5)
        for line in p.stdout.splitlines():
            if " at " in line:
                return line.split(" at ", 1)[-1].strip()
        for line in p.stderr.splitlines():
            if "already mounted at `" in line:
                return line.split("already mounted at `", 1)[-1].split("'", 1)[0].strip()
    except Exception:
        pass
    try:
        p = subprocess.run(["lsblk", "-no", "MOUNTPOINT", dev], capture_output=True, text=True, timeout=2)
        lines = [l.strip() for l in p.stdout.splitlines() if l.strip()]
        if lines:
            return lines[0]
    except Exception:
        pass
    return dev

def get_places():
    home = os.path.expanduser("~")
    places = [
        {"name": "Home", "icon": "home", "path": home},
        {"name": "Documents", "icon": "description", "path": os.path.join(home, "Documents")},
        {"name": "Downloads", "icon": "download", "path": os.path.join(home, "Downloads")},
        {"name": "Pictures", "icon": "photo", "path": os.path.join(home, "Pictures")},
        {"name": "Music", "icon": "music_note", "path": os.path.join(home, "Music")},
    ]
    places = [p for p in places if os.path.isdir(p["path"])]

    # Detected block devices / external drives
    try:
        p = subprocess.run(["lsblk", "-J", "-o", "NAME,LABEL,MOUNTPOINTS,FSTYPE,SIZE"], capture_output=True, text=True, timeout=2)
        if p.returncode == 0:
            data = json.loads(p.stdout)
            def scan_devs(devices):
                res = []
                for d in devices:
                    lbl = d.get("label")
                    # Ignore swap, boot, and internal system partitions
                    if lbl and lbl not in ["SYSTEM", "linux-boot", "linux-swap", "zram0", "RESTORE", "MYASUS", "linux-root"]:
                        mps = [m for m in (d.get("mountpoints") or []) if m and not m.startswith("[")]
                        mp = mps[0] if mps else ""
                        if mp not in ["/", "/home", "/boot/efi", "/boot", "/var/tmp"]:
                            res.append({
                                "name": lbl,
                                "icon": "hard_drive",
                                "dev": "/dev/" + d["name"],
                                "path": mp or ("/dev/" + d["name"]),
                                "is_disk": True,
                                "is_mounted": bool(mp)
                            })
                    if "children" in d:
                        res.extend(scan_devs(d["children"]))
                return res
            places.extend(scan_devs(data.get("blockdevices", [])))
    except Exception:
        pass

    places.append({"name": "Root (/)", "icon": "computer", "path": "/"})
    return places

def list_dir(target):
    target = os.path.expanduser((target or "~").strip())
    
    # If target is a block device, mount it first
    if target.startswith("/dev/"):
        target = mount_dev(target)

    # If target path is under /media or /run/media and doesn't exist yet, attempt mounting
    if not os.path.exists(target) and "/media" in target:
        parts = target.split("/media/", 1)[-1].split("/")
        vol_name = parts[1] if len(parts) > 1 else parts[0]
        try:
            p = subprocess.run(["lsblk", "-J", "-o", "NAME,LABEL,MOUNTPOINTS"], capture_output=True, text=True)
            if p.returncode == 0:
                data = json.loads(p.stdout)
                def find_and_mount(devices):
                    for d in devices:
                        if d.get("label") == vol_name:
                            return mount_dev("/dev/" + d["name"])
                        if "children" in d:
                            m = find_and_mount(d["children"])
                            if m: return m
                    return None
                find_and_mount(data.get("blockdevices", []))
        except Exception:
            pass

    if not os.path.isdir(target):
        target = os.path.expanduser("~")

    abs_path = os.path.abspath(target)
    parent = os.path.dirname(abs_path)
    
    # Compute breadcrumb segments
    crumbs = []
    curr = abs_path
    while curr and curr != "/":
        crumbs.insert(0, {"name": os.path.basename(curr), "path": curr})
        curr = os.path.dirname(curr)
    crumbs.insert(0, {"name": "Root (/)", "path": "/"})

    folders = []
    ignored = {"$RECYCLE.BIN", "System Volume Information"}
    try:
        with os.scandir(abs_path) as it:
            for entry in it:
                try:
                    if entry.is_dir(follow_symlinks=True) and not entry.name.startswith(".") and entry.name not in ignored:
                        folders.append({
                            "name": entry.name,
                            "path": entry.path
                        })
                except Exception:
                    pass
    except Exception as e:
        return {"error": str(e), "path": abs_path, "parent": parent, "crumbs": crumbs, "folders": []}

    folders.sort(key=lambda x: x["name"].lower())
    return {
        "path": abs_path,
        "parent": parent,
        "name": os.path.basename(abs_path) or "/",
        "crumbs": crumbs,
        "folders": folders
    }

if __name__ == "__main__":
    action = sys.argv[1] if len(sys.argv) > 1 else "list"
    arg = sys.argv[2] if len(sys.argv) > 2 else "~"
    
    if action == "places":
        print(json.dumps(get_places()))
    elif action == "mount":
        print(json.dumps({"path": mount_dev(arg)}))
    else:
        print(json.dumps(list_dir(arg)))
