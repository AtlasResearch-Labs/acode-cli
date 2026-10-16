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

def run_task(task, acode_bin, model=None, provider=None, timeout_secs=180):
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

        # Isolate credentials: pass environment variables through child_env instead of copying .env file to disk
        child_env = os.environ.copy()
        workspace_env = Path(__file__).resolve().parent.parent / ".env"
        if workspace_env.exists():
            with open(workspace_env, "r", encoding="utf-8") as f:
                for line in f:
                    line = line.strip()
                    if line and not line.startswith("#") and "=" in line:
                        k, v = line.split("=", 1)
                        child_env[k.strip()] = v.strip().strip("'").strip('"')

        # Run acode non-interactively with optional model/provider specification
        start_time = time.time()
        cmd = [acode_bin, "--security", "full"]
        if model:
            cmd.extend(["--model", model])
        if provider:
            cmd.extend(["--provider", provider])
        cmd.append(prompt)
        
        proc = subprocess.run(
            cmd,
            cwd=temp_dir,
            env=child_env,
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
                env=child_env,
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
    import argparse
    parser = argparse.ArgumentParser(description="Atlas Daedalus Standardized Benchmark Evaluation Harness")
    parser.add_argument("--model", type=str, default=None, help="Explicit foundation model (e.g. gemini-2.5-flash, deepseek-chat, gpt-4o)")
    parser.add_argument("--provider", type=str, default=None, help="Provider backend (gemini, openai, anthropic, powerbox, ollama)")
    parser.add_argument("--runs", type=int, default=1, help="Number of benchmark evaluation runs (default: 1)")
    args = parser.parse_args()

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

    named_model = args.model or "environment_configured"
    named_provider = args.provider or "environment_configured"

    print("=" * 65)
    print(" Atlas Daedalus Standardized Benchmark Evaluation Harness")
    print(f" Binary:   {acode_bin}")
    print(f" Provider: {named_provider}")
    print(f" Model:    {named_model}")
    print(f" Tasks:    {len(tasks)} benchmark problems")
    print(f" Runs:     {args.runs}")
    print("=" * 65)

    all_runs = []
    total_passed = 0
    total_attempts = len(tasks) * args.runs

    for run_idx in range(1, args.runs + 1):
        if args.runs > 1:
            print(f"\n--- TRIAL RUN {run_idx}/{args.runs} ---")

        run_results = []
        for idx, task in enumerate(tasks, start=1):
            print(f"[{idx}/{len(tasks)}] Running {task['id']}: {task['name']}...", end=" ", flush=True)
            res = run_task(task, acode_bin, model=args.model, provider=args.provider)
            run_results.append(res)

            if res["passed"]:
                total_passed += 1
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

        all_runs.append({
            "run_index": run_idx,
            "tasks": run_results
        })

    overall_pass_rate = (total_passed / total_attempts) * 100.0 if total_attempts else 0.0

    print("=" * 65)
    print(f" Benchmark Summary: {total_passed}/{total_attempts} attempts passed ({overall_pass_rate:.1f}% pass@1)")
    print("=" * 65)

    report = {
        "timestamp": int(time.time()),
        "acode_binary": acode_bin,
        "model": named_model,
        "provider": named_provider,
        "runs": args.runs,
        "total_tasks": len(tasks),
        "total_attempts": total_attempts,
        "passed_attempts": total_passed,
        "pass_rate_percent": overall_pass_rate,
        "evaluation_trials": all_runs
    }

    results_file = eval_dir / "results.json"
    with open(results_file, "w", encoding="utf-8") as f:
        json.dump(report, f, indent=2)

    print(f"Detailed results saved to: {results_file}")

if __name__ == "__main__":
    main()
