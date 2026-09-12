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

CACHE_FILE = "/tmp/radial_tabs_cache.json"

def get_hypr_socket():
    runtime_dir = os.environ.get("XDG_RUNTIME_DIR", f"/run/user/{os.getuid()}")
    sig = os.environ.get("HYPRLAND_INSTANCE_SIGNATURE", "")
    if sig:
        sock = os.path.join(runtime_dir, "hypr", sig, ".socket.sock")
        if os.path.exists(sock):
            return sock

    # Fallback: Scan runtime_dir/hypr for newest valid socket
    hypr_dir = os.path.join(runtime_dir, "hypr")
    if os.path.isdir(hypr_dir):
        valid_socks = []
        for d in os.listdir(hypr_dir):
            candidate = os.path.join(hypr_dir, d, ".socket.sock")
            if os.path.exists(candidate):
                try:
                    valid_socks.append((os.path.getmtime(candidate), candidate))
                except OSError:
                    pass
        if valid_socks:
            valid_socks.sort(key=lambda x: x[0], reverse=True)
            return valid_socks[0][1]

    return os.path.join(runtime_dir, "hypr", sig or "default", ".socket.sock")

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

def get_gpu_profile():
    cache_path = "/tmp/radial_gpu_profile.json"
    now = time.time()
    if os.path.exists(cache_path):
        try:
            if now - os.path.getmtime(cache_path) < 3600:
                with open(cache_path, "r") as f:
                    return json.load(f)
        except Exception:
            pass

    # Check user override in config.json
    cfg_path = os.path.expanduser("~/.config/radialMenu/config.json")
    forced_profile = None
    if os.path.exists(cfg_path):
        try:
            with open(cfg_path, "r") as f:
                cfg = json.load(f)
                perf = cfg.get("performance", {})
                forced_profile = perf.get("profile", "auto")
        except Exception:
            pass

    if forced_profile in ("low", "low_end"):
        profile = {
            "is_low_end": True,
            "has_discrete_gpu": False,
            "gpu_acceleration": False,
            "profile": "low_end",
            "reason": "Config override: low_end"
        }
    elif forced_profile in ("high", "high_performance"):
        profile = {
            "is_low_end": False,
            "has_discrete_gpu": True,
            "gpu_acceleration": True,
            "profile": "high_performance",
            "reason": "Config override: high_performance"
        }
    else:
        # Hardware auto-detection
        import glob
        drm_vendors = []
        for vendor_path in glob.glob("/sys/class/drm/card[0-9]*/device/vendor"):
            try:
                with open(vendor_path, "r") as f:
                    drm_vendors.append(f.read().strip().lower())
            except Exception:
                pass

        has_nvidia = any("0x10de" in v for v in drm_vendors)
        has_discrete = has_nvidia
        if not has_discrete:
            try:
                p = subprocess.run(["lspci", "-nn"], capture_output=True, text=True, timeout=0.15)
                for line in p.stdout.splitlines():
                    low = line.lower()
                    if any(k in low for k in ["vga compatible controller", "3d controller", "display controller"]):
                        if any(dgpu in low for dgpu in ["nvidia", "geforce", "quadro", "rtx", "arc", "radeon rx", "radeon pro"]):
                            has_discrete = True
                            break
            except Exception:
                pass

        profile = {
            "is_low_end": not has_discrete,
            "has_discrete_gpu": has_discrete,
            "gpu_acceleration": has_discrete,
            "profile": "high_performance" if has_discrete else "low_end",
            "reason": "Dedicated GPU detected" if has_discrete else "Integrated / low-power GPU detected"
        }

    try:
        with open(cache_path, "w") as f:
            json.dump(profile, f)
    except Exception:
        pass
    return profile

def get_cached_tabs(allow_spawn=True):
    # 1. Real-time shared memory tabs (0ms from browser extension)
    shm_file = "/dev/shm/browser_tabs.json"
    if os.path.exists(shm_file):
        try:
            with open(shm_file, "r") as f:
                data = json.load(f)
                if isinstance(data, list) and len(data) > 0:
                    return data
        except Exception:
            pass

    # 2. Check disk cache (fresh within 5s)
    now = time.time()
    if os.path.exists(CACHE_FILE):
        try:
            if now - os.path.getmtime(CACHE_FILE) < 5.0:
                with open(CACHE_FILE, "r") as f:
                    return json.load(f)
        except Exception:
            pass

    # 3. If stale and allowed, trigger background refresh without blocking
    if allow_spawn:
        try:
            script_dir = os.path.dirname(os.path.abspath(__file__))
            tabs_script = os.path.join(script_dir, "get_browser_tabs.py")
            if os.path.exists(tabs_script):
                subprocess.Popen(["python3", tabs_script], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        except Exception:
            pass

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
    gpu = get_gpu_profile()
    print(json.dumps({
        "cursor": cursor,
        "window": win,
        "tabs": tabs,
        "clients": clients,
        "gpu": gpu
    }))

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
    elif cmd == "clipboard":
        cmd_clipboard()
    elif cmd == "audio_sinks":
        cmd_audio_sinks()
    elif cmd == "gpu":
        print(json.dumps(get_gpu_profile(), indent=2))
    elif cmd == "paste" and len(sys.argv) >= 3:
        cmd_paste(sys.argv[2])
    elif cmd == "set_sink" and len(sys.argv) >= 3:
        cmd_set_sink(sys.argv[2])
    else:
        cmd_context()
