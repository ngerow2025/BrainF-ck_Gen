import subprocess
import time
import re
import os
import json
import sys
from pathlib import Path

RUN_RS_PATH = Path("src/run.rs")
STATE_PATH = Path("latest_bounds.json")

LOWER_BOUND = 1
UPPER_BOUND = 4294967296
RETRIES = 2
MAX_RESTARTS = 5
RESTART_DELAY = 5  # seconds
EXECUTABLE = "./target/release/bf_opt"

CONST_PATTERN = re.compile(r"const SHRINK_TO_SIZE: usize = \d+;")

def log(msg, log_path):
    timestamp = time.strftime("[%Y-%m-%d %H:%M:%S]")
    with open(log_path, "a") as f:
        f.write(f"{timestamp} {msg}\n")
    print(f"{timestamp} {msg}")

def update_const(value):
    code = RUN_RS_PATH.read_text()
    updated = CONST_PATTERN.sub(f"const SHRINK_TO_SIZE: usize = {value};", code)
    RUN_RS_PATH.write_text(updated)

def build_project(log_path):
    try:
        result = subprocess.run(["cargo", "build", "--release"],
                                stdout=subprocess.PIPE,
                                stderr=subprocess.PIPE,
                                timeout=300)
        if result.returncode != 0:
            log(f"Build stderr: {result.stderr.decode()}", log_path)
        return result.returncode == 0
    except Exception as e:
        log(f"Build exception: {e}", log_path)
        return False

def run_and_time(log_path):
    times = []
    for attempt in range(3):
        try:
            start = time.time()
            result = subprocess.run([EXECUTABLE], stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=800)
            end = time.time()
            if result.returncode == 0:
                times.append(end - start)
            else:
                log(f"Run failed (attempt {attempt + 1}) with return code {result.returncode}. stderr: {result.stderr.decode()}", log_path)
        except subprocess.TimeoutExpired:
            log(f"Run timed out on attempt {attempt + 1}.", log_path)
        except Exception as e:
            log(f"Run exception on attempt {attempt + 1}: {e}", log_path)
    if times:
        return min(times)
    return None

def save_state(low, high, best_val, best_time):
    state = {
        "low": low,
        "high": high,
        "best_val": best_val,
        "best_time": best_time
    }
    with open(STATE_PATH, "w") as f:
        json.dump(state, f)

def load_state():
    if STATE_PATH.exists():
        try:
            with open(STATE_PATH) as f:
                state = json.load(f)
            return (
                state.get("low", LOWER_BOUND),
                state.get("high", UPPER_BOUND),
                state.get("best_val"),
                state.get("best_time", float("inf"))
            )
        except Exception as e:
            log(f"Error reading state file: {e}")
    return LOWER_BOUND, UPPER_BOUND, None, float("inf")

def stepwise_search(log_path):
    low, high, best_val, best_time = load_state()
    if best_val is None:
        best_val = (low + high) // 2
        best_time = float("inf")

    step = max(1, (high - low) // 4)
    current = best_val

    while step >= 1:
        candidates = [current - step, current, current + step]
        candidates = [c for c in candidates if low <= c <= high]
        times = {}

        for val in candidates:
            log(f"Testing SHRINK_TO_SIZE = {val}", log_path)
            update_const(val)
            if not build_project(log_path):
                log("Build failed. Skipping value.", log_path)
                continue
            runtime = run_and_time(log_path)
            if runtime is not None:
                times[val] = runtime
                log(f"Success: SHRINK_TO_SIZE = {val}, Time = {runtime:.2f} seconds", log_path)
            else:
                log("Execution failed. Skipping value.", log_path)

        if not times:
            log("No successful runs in this step. Halving step size.", log_path)
            step //= 2
            continue

        min_val = min(times, key=times.get)
        min_time = times[min_val]

        # Find all candidates within 2% of the best time
        close_vals = [v for v, t in times.items() if abs(t - min_time) / min_time < 0.02]
        if len(close_vals) > 1:
            # Focus search between the min and max of these close candidates
            new_low = max(low, min(close_vals))
            new_high = min(high, max(close_vals))
            log(f"Plateau detected between {new_low} and {new_high}, reducing step.", log_path)
            low, high = new_low, new_high
            current = (low + high) // 2
            step = max(1, step // 2)
        elif min_time < best_time:
            best_time = min_time
            best_val = min_val
            log(f"New best: SHRINK_TO_SIZE = {best_val}, Time = {best_time:.2f} seconds", log_path)
            current = best_val
            # keep step the same to keep exploring in this direction
        else:
            # No improvement, reduce step size
            step //= 2

        save_state(low, high, best_val, best_time)

    # Local sweep around best_val
    sweep_range = range(max(low, best_val - 2), min(high, best_val + 3))
    for val in sweep_range:
        log(f"Local sweep: Testing SHRINK_TO_SIZE = {val}", log_path)
        update_const(val)
        if not build_project(log_path):
            log("Build failed. Skipping value.", log_path)
            continue
        runtime = run_and_time(log_path)
        if runtime is not None and runtime < best_time:
            best_time = runtime
            best_val = val
            log(f"Local sweep new best: SHRINK_TO_SIZE = {best_val}, Time = {best_time:.2f} seconds", log_path)
            save_state(low, high, best_val, best_time)

    log(f"Search complete. Best SHRINK_TO_SIZE = {best_val} with time = {best_time:.2f}s", log_path)

def main():
    attempt = 0
    while True:
        log_path = Path(f"stepwise_search_log_{attempt + 1}.txt")
        log(f"===== Stepwise Search Attempt #{attempt + 1} Started =====", log_path)
        try:
            stepwise_search(log_path)
        except Exception as e:
            log(f"Fatal error: {e}", log_path)
            if attempt < MAX_RESTARTS:
                log(f"Restarting in {RESTART_DELAY} seconds...", log_path)
                time.sleep(RESTART_DELAY)
                attempt += 1
                continue
            else:
                log("Max restarts reached. Exiting.", log_path)
                break
        log("===== Stepwise Search Finished =====", log_path)
        attempt += 1
        # Optionally, sleep or reset state here if needed

if __name__ == "__main__":
    main()
