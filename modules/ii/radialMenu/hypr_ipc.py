#!/usr/bin/env python3
"""
High-Performance UNIX Socket IPC & Helper for Radial Dial.
Provides sub-millisecond querying for Hyprland window context, active clients,
clipboard history, and PipeWire audio sinks.
"""

import sys
import os
import socket
import json
import subprocess
import time
from pathlib import Path

CACHE_FILE = "/tmp/radial_tabs_cache.json"

HIGH_POWER_VRAM_BYTES = 2 * 1024 * 1024 * 1024

def detect_render_mode():
    """Return ``gpu`` only for a dedicated/high-performance graphics device.

    Every desktop has a GPU, so treating any VGA controller as high power would
    incorrectly enable the expensive path on integrated Intel/AMD graphics.
    Prefer DRM device metadata, with lspci names as a fallback.
    """
    drm_devices = Path("/sys/class/drm").glob("card[0-9]*/device")
    for device in drm_devices:
        try:
            vendor = (device / "vendor").read_text().strip().lower()
        except OSError:
            continue

        # NVIDIA DRM devices are discrete on the supported laptop/desktop
        # platforms. AMD is considered high power when it exposes >= 2 GiB of
        # dedicated VRAM; integrated AMD graphics normally use shared memory.
        if vendor == "0x10de":
            return "gpu"
        if vendor == "0x1002":
            try:
                vram = int((device / "mem_info_vram_total").read_text().strip())
                if vram >= HIGH_POWER_VRAM_BYTES:
                    return "gpu"
            except (OSError, ValueError):
                pass

    try:
        result = subprocess.run(
            ["lspci", "-nn"], capture_output=True, text=True, timeout=0.5,
            check=False
        )
        adapters = "\n".join(
            line.lower() for line in result.stdout.splitlines()
            if "vga compatible controller" in line.lower()
            or "3d controller" in line.lower()
            or "display controller" in line.lower()
        )
        if "nvidia" in adapters or ("intel" in adapters and " arc" in adapters):
            return "gpu"
        if "amd" in adapters or "ati" in adapters:
            high_power_amd_names = ("radeon rx", "radeon pro w", "radeon instinct")
            if any(name in adapters for name in high_power_amd_names):
                return "gpu"
    except (OSError, subprocess.SubprocessError):
        pass

    return "low-power"

def get_hypr_socket():
    runtime_dir = os.environ.get("XDG_RUNTIME_DIR", f"/run/user/{os.getuid()}")
    sig = os.environ.get("HYPRLAND_INSTANCE_SIGNATURE", "")
    if not sig:
        # Scan runtime_dir/hypr for signature folder
        hypr_dir = os.path.join(runtime_dir, "hypr")
        if os.path.isdir(hypr_dir):
            subdirs = [d for d in os.listdir(hypr_dir) if os.path.isdir(os.path.join(hypr_dir, d))]
            if subdirs:
                sig = subdirs[0]
    return os.path.join(runtime_dir, "hypr", sig, ".socket.sock")

def query_hypr_socket(cmd: str):
    sock_path = get_hypr_socket()
    if not os.path.exists(sock_path):
        return None
    try:
        s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        s.settimeout(0.15)
        s.connect(sock_path)
        s.sendall(cmd.encode("utf-8"))
        buf = b""
        while True:
            chunk = s.recv(8192)
            if not chunk:
                break
            buf += chunk
        s.close()
        return json.loads(buf.decode("utf-8", errors="ignore"))
    except Exception:
        return None

def get_cached_tabs():
    script_dir = os.path.dirname(os.path.abspath(__file__))
    tabs_script = os.path.join(script_dir, "get_browser_tabs.py")
    if not os.path.exists(tabs_script):
        return []
    # Check cache freshness (500ms)
    now = time.time()
    if os.path.exists(CACHE_FILE):
        try:
            mtime = os.path.getmtime(CACHE_FILE)
            if now - mtime < 0.8:
                with open(CACHE_FILE, "r") as f:
                    return json.load(f)
        except Exception:
            pass
    try:
        p = subprocess.run(["python3", tabs_script], capture_output=True, text=True, timeout=0.3)
        tabs = json.loads(p.stdout.trim()) if hasattr(p.stdout, 'trim') else json.loads(p.stdout.strip() or "[]")
        with open(CACHE_FILE, "w") as f:
            json.dump(tabs, f)
        return tabs
    except Exception:
        return []

def get_valid_clients():
    clients = query_hypr_socket("j/clients") or []
    valid = []
    for c in clients:
        if not c.get("mapped", False):
            continue
        ws = c.get("workspace", {})
        ws_name = str(ws.get("name", ws.get("id", "")))
        if not ws_name or ws_name.startswith("special:quickshell"):
            continue
        valid.append({
            "address": c.get("address", ""),
            "class": c.get("class", ""),
            "title": c.get("title", ""),
            "workspace": ws_name,
            "workspaceId": ws.get("id", 1),
            "pid": c.get("pid", 0)
        })
    return valid

def cmd_context():
    cursor = query_hypr_socket("j/cursorpos") or {"x": 0, "y": 0}
    win = query_hypr_socket("j/activewindow") or {}
    tabs = get_cached_tabs()
    clients = get_valid_clients()
    print(json.dumps({
        "cursor": cursor,
        "window": win,
        "tabs": tabs,
        "clients": clients
    }))

def cmd_tabs():
    print(json.dumps(get_cached_tabs()))

def cmd_render_mode():
    print(detect_render_mode())

def cmd_clients():
    print(json.dumps(get_valid_clients()))

def cmd_clipboard():
    try:
        p = subprocess.run(["cliphist", "list"], capture_output=True, text=True, timeout=0.3)
        items = []
        for line in p.stdout.splitlines()[:8]:
            parts = line.split("\t", 1)
            if len(parts) == 2:
                cid, text = parts
                preview = text.strip()
                if len(preview) > 36:
                    preview = preview[:33] + "..."
                items.append({"id": cid.strip(), "preview": preview, "full": text.strip()})
        print(json.dumps(items))
    except Exception:
        print("[]")

def cmd_audio_sinks():
    try:
        import re
        p = subprocess.run(["wpctl", "status"], capture_output=True, text=True, timeout=0.3)
        lines = p.stdout.splitlines()
        in_sinks = False
        sinks = []
        for line in lines:
            if "Sinks:" in line:
                in_sinks = True
                continue
            if in_sinks:
                if "Sources:" in line or "Streams:" in line or (line.strip() == "" and len(sinks) > 0):
                    if "│" not in line or "Sources:" in line:
                        break
                m = re.search(r'(\*?)\s*(\d+)\.\s+([^\[]+)', line)
                if m:
                    is_def = (m.group(1) == "*")
                    sid = m.group(2)
                    name = m.group(3).strip()
                    # Clean up common device names
                    clean_name = name
                    for prefix in ["Built-in Audio ", "Family 17h/19h HD Audio Controller "]:
                        clean_name = clean_name.replace(prefix, "")
                    sinks.append({"id": sid, "name": clean_name[:24], "fullName": name, "default": is_def})
        print(json.dumps(sinks))
    except Exception:
        print("[]")

def cmd_paste(clip_id):
    try:
        p1 = subprocess.Popen(["cliphist", "decode", str(clip_id)], stdout=subprocess.PIPE)
        subprocess.run(["wl-copy"], stdin=p1.stdout, timeout=0.5)
        p1.wait()
        # Brief pause to let window receive focus before pasting
        time.sleep(0.08)
        subprocess.run(["wtype", "-M", "ctrl", "-k", "v", "-m", "ctrl"], timeout=0.5)
    except Exception:
        pass

def cmd_set_sink(sink_id):
    try:
        subprocess.run(["wpctl", "set-default", str(sink_id)], timeout=0.5)
    except Exception:
        pass

if __name__ == "__main__":
    if len(sys.argv) < 2:
        cmd_context()
        sys.exit(0)
    cmd = sys.argv[1]
    if cmd == "context":
        cmd_context()
    elif cmd == "clients":
        cmd_clients()
    elif cmd == "tabs":
        cmd_tabs()
    elif cmd == "render_mode":
        cmd_render_mode()
    elif cmd == "clipboard":
        cmd_clipboard()
    elif cmd == "audio_sinks":
        cmd_audio_sinks()
    elif cmd == "paste" and len(sys.argv) >= 3:
        cmd_paste(sys.argv[2])
    elif cmd == "set_sink" and len(sys.argv) >= 3:
        cmd_set_sink(sys.argv[2])
    else:
        cmd_context()
