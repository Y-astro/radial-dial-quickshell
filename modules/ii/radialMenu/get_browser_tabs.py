#!/usr/bin/env python3
import glob, os, struct, json, sys, time

SHM_FILE = "/dev/shm/browser_tabs.json"

def get_shm_tabs():
    try:
        if os.path.exists(SHM_FILE):
            # Check if file has data
            with open(SHM_FILE, "r") as f:
                data = json.load(f)
                if isinstance(data, list) and len(data) > 0:
                    return data
    except Exception:
        pass
    return None

def decompress_mozlz4(raw_bytes):
    dest = bytearray()
    pos = 0
    while pos < len(raw_bytes):
        token = raw_bytes[pos]
        pos += 1
        lit_len = token >> 4
        if lit_len == 15:
            while True:
                s = raw_bytes[pos]
                pos += 1
                lit_len += s
                if s != 255:
                    break
        dest.extend(raw_bytes[pos:pos+lit_len])
        pos += lit_len
        if pos >= len(raw_bytes):
            break
        offset = raw_bytes[pos] | (raw_bytes[pos+1] << 8)
        pos += 2
        match_len = (token & 0x0f) + 4
        if match_len == 19:
            while True:
                s = raw_bytes[pos]
                pos += 1
                match_len += s
                if s != 255:
                    break
        for _ in range(match_len):
            dest.append(dest[-offset])
    return bytes(dest)

def get_firefox_session_tabs():
    patterns = [
        os.path.expanduser('~/.config/mozilla/firefox/*/sessionstore-backups/recovery.jsonlz4'),
        os.path.expanduser('~/.mozilla/firefox/*/sessionstore-backups/recovery.jsonlz4'),
        os.path.expanduser('~/.var/app/org.mozilla.firefox/.mozilla/firefox/*/sessionstore-backups/recovery.jsonlz4'),
        os.path.expanduser('~/.zen/*/sessionstore-backups/recovery.jsonlz4'),
        os.path.expanduser('~/.var/app/app.zen_browser.zen/.zen/*/sessionstore-backups/recovery.jsonlz4'),
    ]
    files = []
    for pat in patterns:
        files.extend(glob.glob(pat))
    
    if not files:
        return []
    
    files.sort(key=os.path.getmtime, reverse=True)
    target_file = files[0]
    
    try:
        with open(target_file, 'rb') as f:
            magic = f.read(8)
            if magic != b'mozLz40\0':
                return []
            _ = struct.unpack('<I', f.read(4))[0]
            compressed = f.read()
            decomp = decompress_mozlz4(compressed)
            session = json.loads(decomp.decode('utf-8'))
            
            tabs = []
            for win in session.get('windows', []):
                for i, tab in enumerate(win.get('tabs', [])):
                    entries = tab.get('entries', [])
                    idx = tab.get('index', len(entries)) - 1
                    if entries and 0 <= idx < len(entries):
                        title = entries[idx].get('title', f'Tab {i+1}')
                        url = entries[idx].get('url', '')
                    else:
                        title = f'Tab {i+1}'
                        url = ''
                    tabs.append({
                        'index': i + 1,
                        'title': title,
                        'url': url
                    })
            return tabs
    except Exception:
        return []

if __name__ == '__main__':
    # 1. Try real-time shared memory tabs (0ms latency)
    tabs = get_shm_tabs()
    # 2. Fallback to sessionstore
    if not tabs:
        tabs = get_firefox_session_tabs()
    if tabs:
        try:
            with open("/tmp/radial_tabs_cache.json", "w") as f:
                json.dump(tabs, f)
        except Exception:
            pass
    print(json.dumps(tabs or []))
