#!/usr/bin/env python3
"""
Atlas Daedalus — Standardized Evaluation Harness
Runs non-interactive benchmark tasks and evaluates pass@1 rate.
(C) 2026 Atlas Research Labs — MIT License
"""

import os
import sys
import json
import time
import shutil
import tempfile
import subprocess
from pathlib import Path

def find_acode_binary():
    # Check debug/release cargo binaries
    workspace_root = Path(__file__).resolve().parent.parent
    candidates = [
        workspace_root / "target" / "release" / "acode.exe",
        workspace_root / "target" / "release" / "acode",
        workspace_root / "target" / "debug" / "acode.exe",
        workspace_root / "target" / "debug" / "acode",
    ]
    for c in candidates:
        if c.exists() and os.access(c, os.X_OK):
            return str(c)

    # Check PATH
    which_bin = shutil.which("acode")
    if which_bin:
        return which_bin

    return None

def run_task(task, acode_bin, timeout_secs=180):
    task_id = task["id"]
    name = task["name"]
    prompt = task["prompt"]
    setup = task.get("workspace_setup", {})
    verify_cmd = task.get("verification_cmd", "")

    temp_dir = tempfile.mkdtemp(prefix=f"acode_eval_{task_id}_")

    try:
        # Write setup files
        for rel_path, content in setup.items():
            full_path = Path(temp_dir) / rel_path
            full_path.parent.mkdir(parents=True, exist_ok=True)
            with open(full_path, "w", encoding="utf-8") as f:
                f.write(content)

        # Copy workspace .env if present
        workspace_env = Path(__file__).resolve().parent.parent / ".env"
        if workspace_env.exists():
            shutil.copy(workspace_env, Path(temp_dir) / ".env")

        # Run acode non-interactively
        start_time = time.time()
        cmd = [acode_bin, "--security", "full", prompt]
        
        proc = subprocess.run(
            cmd,
            cwd=temp_dir,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            timeout=timeout_secs,
            encoding="utf-8",
            errors="replace"
        )
        duration_ms = int((time.time() - start_time) * 1000)

        # Run verification command
        passed = False
        verify_output = ""
        if verify_cmd:
            v_res = subprocess.run(
                verify_cmd,
                shell=True,
                cwd=temp_dir,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                timeout=60,
                encoding="utf-8",
                errors="replace"
            )
            passed = (v_res.returncode == 0)
            verify_output = v_res.stdout + v_res.stderr
        else:
            passed = (proc.returncode == 0)

        return {
            "id": task_id,
            "name": name,
            "passed": passed,
            "duration_ms": duration_ms,
            "acode_exit_code": proc.returncode,
            "verification_output": verify_output.strip(),
            "acode_stdout": proc.stdout.strip()
        }

    except subprocess.TimeoutExpired:
        return {
            "id": task_id,
            "name": name,
            "passed": False,
            "duration_ms": timeout_secs * 1000,
            "acode_exit_code": -1,
            "error": "Task timed out"
        }
    except Exception as e:
        return {
            "id": task_id,
            "name": name,
            "passed": False,
            "duration_ms": 0,
            "acode_exit_code": -1,
            "error": str(e)
        }
    finally:
        shutil.rmtree(temp_dir, ignore_errors=True)

def main():
    eval_dir = Path(__file__).resolve().parent
    tasks_file = eval_dir / "tasks.json"

    if not tasks_file.exists():
        print(f"Error: Tasks file not found at {tasks_file}", file=sys.stderr)
        sys.exit(1)

    with open(tasks_file, "r", encoding="utf-8") as f:
        tasks = json.load(f)

    acode_bin = find_acode_binary()
    if not acode_bin:
        print("Error: Could not locate 'acode' executable. Run 'cargo build --release' first.", file=sys.stderr)
        sys.exit(1)

    print("=" * 65)
    print(" Atlas Daedalus Standardized Benchmark Evaluation Harness")
    print(f" Binary: {acode_bin}")
    print(f" Tasks:  {len(tasks)} benchmark problems")
    print("=" * 65)

    results = []
    passed_count = 0

    for idx, task in enumerate(tasks, start=1):
        print(f"[{idx}/{len(tasks)}] Running {task['id']}: {task['name']}...", end=" ", flush=True)
        res = run_task(task, acode_bin)
        results.append(res)

        if res["passed"]:
            passed_count += 1
            print(f"PASSED ({res['duration_ms']}ms)")
        else:
            print(f"FAILED ({res['duration_ms']}ms)")
            if "error" in res:
                print(f"    Error: {res['error']}")
            elif res.get("verification_output"):
                print(f"    Output: {res['verification_output'][:200]}")
            if "acode_stdout" in res and not res["passed"]:
                print(f"    Agent Output:\n{res['acode_stdout']}")

        if idx < len(tasks):
            time.sleep(2)

    pass_rate = (passed_count / len(tasks)) * 100.0 if tasks else 0.0

    print("=" * 65)
    print(f" Benchmark Summary: {passed_count}/{len(tasks)} passed ({pass_rate:.1f}% pass@1)")
    print("=" * 65)

    report = {
        "timestamp": int(time.time()),
        "acode_binary": acode_bin,
        "total_tasks": len(tasks),
        "passed_tasks": passed_count,
        "pass_rate_percent": pass_rate,
        "tasks": results
    }

    results_file = eval_dir / "results.json"
    with open(results_file, "w", encoding="utf-8") as f:
        json.dump(report, f, indent=2)

    print(f"Detailed results saved to: {results_file}")

if __name__ == "__main__":
    main()
