use serde_json::Value;
use std::fs;
use std::path::PathBuf;

#[test]
fn test_eval_benchmark_suite_integrity() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let tasks_path = manifest_dir.join("eval").join("tasks.json");
    let harness_path = manifest_dir.join("eval").join("harness.py");

    assert!(tasks_path.exists(), "eval/tasks.json must exist");
    assert!(harness_path.exists(), "eval/harness.py must exist");

    let content = fs::read_to_string(&tasks_path).expect("Should read tasks.json");
    let tasks: Value = serde_json::from_str(&content).expect("tasks.json must be valid JSON");

    let arr = tasks
        .as_array()
        .expect("tasks.json must be a JSON array of tasks");
    assert!(
        !arr.is_empty(),
        "Benchmark suite must contain at least one task"
    );

    for task in arr {
        assert!(
            task.get("id").and_then(|v| v.as_str()).is_some(),
            "Task must have 'id'"
        );
        assert!(
            task.get("name").and_then(|v| v.as_str()).is_some(),
            "Task must have 'name'"
        );
        assert!(
            task.get("prompt").and_then(|v| v.as_str()).is_some(),
            "Task must have 'prompt'"
        );
        assert!(
            task.get("workspace_setup")
                .and_then(|v| v.as_object())
                .is_some(),
            "Task must have 'workspace_setup'"
        );
        assert!(
            task.get("verification_cmd")
                .and_then(|v| v.as_str())
                .is_some(),
            "Task must have 'verification_cmd'"
        );
    }
}
