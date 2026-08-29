#!/usr/bin/env python3
import sys, struct, json, os

SHM_FILE = "/dev/shm/browser_tabs.json"

def read_message():
    raw_length = sys.stdin.buffer.read(4)
    if len(raw_length) < 4:
        return None
    message_length = struct.unpack('<I', raw_length)[0]
    raw_data = sys.stdin.buffer.read(message_length)
    if len(raw_data) < message_length:
        return None
    return json.loads(raw_data.decode('utf-8'))

def send_message(message):
    data = json.dumps(message).encode('utf-8')
    sys.stdout.buffer.write(struct.pack('<I', len(data)))
    sys.stdout.buffer.write(data)
    sys.stdout.buffer.flush()

def main():
    while True:
        try:
            msg = read_message()
            if msg is None:
                break
            # msg is a list of tab objects or contains tabs
            tabs = msg if isinstance(msg, list) else msg.get('tabs', [])
            # Atomically write to /dev/shm/browser_tabs.json
            temp_file = f"{SHM_FILE}.tmp"
            with open(temp_file, "w") as f:
                json.dump(tabs, f)
            os.replace(temp_file, SHM_FILE)
            send_message({"status": "ok", "count": len(tabs)})
        except Exception as e:
            try:
                send_message({"status": "error", "message": str(e)})
            except Exception:
                pass
            break

if __name__ == '__main__':
    main()
