#!/usr/bin/env python3
"""Measure Quickshell CPU and radial-menu lifecycle latency on Hyprland."""

from __future__ import annotations

import argparse
import json
import math
import os
from pathlib import Path
import statistics
import subprocess
import sys
import time


RADIAL_NAMESPACE = "quickshell:radialMenu"
RADIAL_DISPATCH = 'hl.dsp.global("quickshell:radialMenu")'


def find_quickshell_pid() -> int:
    candidates: list[tuple[int, int]] = []
    for entry in Path("/proc").iterdir():
        if not entry.name.isdigit():
            continue
        try:
            cmdline = (entry / "cmdline").read_bytes().replace(b"\0", b" ").decode()
            stat = (entry / "stat").read_text()
            fields = stat[stat.rfind(")") + 2 :].split()
            started = int(fields[19])
        except (FileNotFoundError, PermissionError, ProcessLookupError):
            continue
        if "qs -c end4-pC" in cmdline or "quickshell -c end4-pC" in cmdline:
            candidates.append((started, int(entry.name)))
    if not candidates:
        raise RuntimeError("No running end4-pC Quickshell process found")
    return max(candidates)[1]


def process_ticks(pid: int) -> int:
    stat = Path(f"/proc/{pid}/stat").read_text()
    fields = stat[stat.rfind(")") + 2 :].split()
    return int(fields[11]) + int(fields[12])


def rss_kib(pid: int) -> int:
    for line in Path(f"/proc/{pid}/status").read_text().splitlines():
        if line.startswith("VmRSS:"):
            return int(line.split()[1])
    return 0


def hyprctl(*args: str) -> str:
    return subprocess.run(
        ["hyprctl", *args], check=True, capture_output=True, text=True
    ).stdout


def menu_visible() -> bool:
    return RADIAL_NAMESPACE in hyprctl("layers")


def wait_for_visibility(expected: bool, timeout: float = 2.0) -> float:
    started = time.monotonic()
    while time.monotonic() - started < timeout:
        if menu_visible() is expected:
            return (time.monotonic() - started) * 1000.0
        time.sleep(0.02)
    raise RuntimeError(f"Radial menu visibility did not become {expected}")


def toggle_menu(expected: bool) -> float:
    if menu_visible() is expected:
        return 0.0
    started = time.monotonic()
    hyprctl("dispatch", RADIAL_DISPATCH)
    wait_for_visibility(expected)
    return (time.monotonic() - started) * 1000.0


def cursor_position() -> tuple[int, int]:
    data = json.loads(hyprctl("cursorpos", "-j"))
    return round(data["x"]), round(data["y"])


def benchmark_anchor(cursor: tuple[int, int]) -> tuple[int, int]:
    monitors = json.loads(hyprctl("monitors", "-j"))
    monitor = next(
        (
            item
            for item in monitors
            if item["x"] <= cursor[0] < item["x"] + item["width"]
            and item["y"] <= cursor[1] < item["y"] + item["height"]
        ),
        monitors[0],
    )
    return (
        round(monitor["x"] + monitor["width"] / 2),
        round(monitor["y"] + monitor["height"] / 2),
    )


def move_cursor(x: int, y: int) -> None:
    # This Hyprland build exposes dispatchers through its Lua API.  The
    # standard `dispatch movecursor x y` spelling is parsed as invalid Lua.
    hyprctl(
        "dispatch",
        f"hl.dsp.cursor.move({{ x = {x}, y = {y} }})",
    )


def pointer_button(pressed: bool) -> None:
    subprocess.run(
        ["ydotool", "click", "0x40" if pressed else "0x80"],
        check=True,
        capture_output=True,
        text=True,
    )


def percentile(values: list[float], fraction: float) -> float:
    ordered = sorted(values)
    if not ordered:
        return 0.0
    index = max(0, math.ceil(len(ordered) * fraction) - 1)
    return ordered[index]


def summarize(values: list[float]) -> dict[str, float]:
    if not values:
        return {"mean": 0.0, "median": 0.0, "p95": 0.0, "min": 0.0, "max": 0.0}
    return {
        "mean": round(statistics.fmean(values), 3),
        "median": round(statistics.median(values), 3),
        "p95": round(percentile(values, 0.95), 3),
        "min": round(min(values), 3),
        "max": round(max(values), 3),
    }


def positive_float(value: str) -> float:
    parsed = float(value)
    if parsed <= 0:
        raise argparse.ArgumentTypeError("must be greater than zero")
    return parsed


def positive_int(value: str) -> int:
    parsed = int(value)
    if parsed <= 0:
        raise argparse.ArgumentTypeError("must be greater than zero")
    return parsed


def nonnegative_float(value: str) -> float:
    parsed = float(value)
    if parsed < 0:
        raise argparse.ArgumentTypeError("must be zero or greater")
    return parsed


def nonnegative_int(value: str) -> int:
    parsed = int(value)
    if parsed < 0:
        raise argparse.ArgumentTypeError("must be zero or greater")
    return parsed


def start_load(workers: int) -> list[subprocess.Popen[bytes]]:
    processes: list[subprocess.Popen[bytes]] = []
    try:
        for _ in range(workers):
            processes.append(
                subprocess.Popen(
                    ["sha256sum", "/dev/zero"],
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL,
                )
            )
    except Exception:
        stop_load(processes)
        raise
    return processes


def stop_load(processes: list[subprocess.Popen[bytes]]) -> None:
    for process in processes:
        process.terminate()
    for process in processes:
        try:
            process.wait(timeout=2)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()


def sample_cpu(
    pid: int,
    duration: float,
    interval: float,
    motion_center: tuple[int, int] | None,
    drag_motion: bool = False,
) -> list[float]:
    clock_ticks = os.sysconf("SC_CLK_TCK")
    samples: list[float] = []
    previous_ticks = process_ticks(pid)
    previous_time = time.monotonic()
    deadline = previous_time + duration
    step = 0

    while time.monotonic() < deadline:
        if motion_center is not None:
            if drag_motion:
                angle = math.radians(22.5 + 6.0 * math.sin(step * math.pi / 3.0))
            else:
                angle = step * math.pi / 6.0
            x = round(motion_center[0] + 112 * math.cos(angle))
            y = round(motion_center[1] + 112 * math.sin(angle))
            move_cursor(x, y)
            step += 1

        time.sleep(interval)
        now = time.monotonic()
        ticks = process_ticks(pid)
        elapsed = now - previous_time
        samples.append((ticks - previous_ticks) / clock_ticks / elapsed * 100.0)
        previous_ticks = ticks
        previous_time = now

    return samples


def run(args: argparse.Namespace) -> dict[str, object]:
    pid = args.pid or find_quickshell_pid()
    original_cursor = cursor_position()
    anchor = benchmark_anchor(original_cursor)
    load_processes = start_load(args.load_workers)
    cpu_samples: list[float] = []
    repetition_cpu: list[float] = []
    open_latencies: list[float] = []
    close_latencies: list[float] = []
    failures: list[str] = []
    pointer_is_down = False

    try:
        toggle_menu(False)
        for repetition in range(args.repetitions):
            time.sleep(args.settle)
            try:
                if args.mode == "closed":
                    samples = sample_cpu(pid, args.duration, args.interval, None)
                else:
                    move_cursor(*anchor)
                    open_latencies.append(toggle_menu(True))
                    center = cursor_position()
                    if args.mode == "drag":
                        start_angle = math.radians(22.5)
                        move_cursor(
                            round(center[0] + 112 * math.cos(start_angle)),
                            round(center[1] + 112 * math.sin(start_angle)),
                        )
                        pointer_button(True)
                        pointer_is_down = True
                    motion_center = center if args.mode in ("motion", "drag") else None
                    samples = sample_cpu(
                        pid,
                        args.duration,
                        args.interval,
                        motion_center,
                        drag_motion=args.mode == "drag",
                    )
                    if pointer_is_down:
                        pointer_button(False)
                        pointer_is_down = False
                    close_latencies.append(toggle_menu(False))
                cpu_samples.extend(samples)
                repetition_cpu.append(statistics.fmean(samples))
            except (RuntimeError, subprocess.CalledProcessError, ProcessLookupError) as error:
                failures.append(f"repetition {repetition + 1}: {error}")
                if pointer_is_down:
                    try:
                        pointer_button(False)
                    except Exception as cleanup_error:
                        failures.append(
                            f"repetition {repetition + 1} pointer release: {cleanup_error}"
                        )
                    else:
                        pointer_is_down = False
                try:
                    toggle_menu(False)
                except Exception as cleanup_error:
                    failures.append(
                        f"repetition {repetition + 1} menu close: {cleanup_error}"
                    )
    finally:
        try:
            if pointer_is_down:
                try:
                    pointer_button(False)
                except Exception as cleanup_error:
                    failures.append(f"final pointer release: {cleanup_error}")
                else:
                    pointer_is_down = False
            try:
                toggle_menu(False)
            except Exception as cleanup_error:
                failures.append(f"final menu close: {cleanup_error}")
            try:
                move_cursor(original_cursor[0], original_cursor[1])
            except Exception as cleanup_error:
                failures.append(f"final cursor restore: {cleanup_error}")
        finally:
            stop_load(load_processes)

    try:
        if menu_visible():
            failures.append("final verification: radial menu is still visible")
    except Exception as cleanup_error:
        failures.append(f"final menu verification: {cleanup_error}")
    try:
        restored_cursor = cursor_position()
        if restored_cursor != original_cursor:
            failures.append(
                "final verification: cursor is at "
                f"{restored_cursor}, expected {original_cursor}"
            )
    except Exception as cleanup_error:
        failures.append(f"final cursor verification: {cleanup_error}")

    result: dict[str, object] = {
        "mode": args.mode,
        "pid": pid,
        "repetitions": args.repetitions,
        "duration_seconds": args.duration,
        "interval_seconds": args.interval,
        "load_workers": args.load_workers,
        "benchmark_anchor": anchor,
        "quickshell_main_cpu_percent_one_core": summarize(cpu_samples),
        "quickshell_main_cpu_percent_per_repetition": summarize(repetition_cpu),
        "rss_kib": rss_kib(pid),
        "failures": failures,
        "raw_cpu_percent": [round(value, 3) for value in cpu_samples],
    }
    if open_latencies:
        result["open_latency_ms"] = summarize(open_latencies)
        result["raw_open_latency_ms"] = [round(value, 3) for value in open_latencies]
    if close_latencies:
        result["close_latency_ms"] = summarize(close_latencies)
        result["raw_close_latency_ms"] = [round(value, 3) for value in close_latencies]
    return result


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--mode", choices=("closed", "static", "motion", "drag"), required=True
    )
    parser.add_argument("--pid", type=int)
    parser.add_argument("--duration", type=positive_float, default=3.0)
    parser.add_argument("--interval", type=positive_float, default=0.1)
    parser.add_argument("--repetitions", type=positive_int, default=5)
    parser.add_argument("--settle", type=nonnegative_float, default=0.5)
    parser.add_argument("--load-workers", type=nonnegative_int, default=0)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()

    result = run(args)
    encoded = json.dumps(result, indent=2)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(encoded + "\n")
    print(encoded)
    if result["failures"]:
        sys.exit(1)


if __name__ == "__main__":
    main()
