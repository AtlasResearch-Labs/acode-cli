//! Atlas Daedalus — Dynamic Task Graph & Goal Decomposition Engine
//! Converts complex multi-step objectives into verifiable, dynamic sub-task state machines
//! with dynamic re-planning, repair task insertion, and live terminal progress rendering.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Failed(String),
    Skipped,
}

impl TaskStatus {
    pub fn badge(&self) -> &'static str {
        match self {
            TaskStatus::Pending => "[ ]",
            TaskStatus::InProgress => "[►]",
            TaskStatus::Completed => "[✓]",
            TaskStatus::Failed(_) => "[x]",
            TaskStatus::Skipped => "[-]",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubTask {
    pub id: usize,
    pub title: String,
    pub description: String,
    pub status: TaskStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskGraph {
    pub goal: String,
    pub tasks: Vec<SubTask>,
    pub current_index: usize,
}

impl TaskGraph {
    pub fn new(goal: &str) -> Self {
        Self {
            goal: goal.to_string(),
            tasks: Vec::new(),
            current_index: 0,
        }
    }

    pub fn add_step(&mut self, title: &str, description: &str) -> usize {
        let id = self.tasks.len() + 1;
        self.tasks.push(SubTask {
            id,
            title: title.to_string(),
            description: description.to_string(),
            status: TaskStatus::Pending,
        });
        id
    }

    pub fn start_current(&mut self) -> Option<&SubTask> {
        if self.current_index < self.tasks.len() {
            self.tasks[self.current_index].status = TaskStatus::InProgress;
            Some(&self.tasks[self.current_index])
        } else {
            None
        }
    }

    pub fn complete_current(&mut self) {
        if self.current_index < self.tasks.len() {
            self.tasks[self.current_index].status = TaskStatus::Completed;
            self.current_index += 1;
            if self.current_index < self.tasks.len() {
                self.tasks[self.current_index].status = TaskStatus::InProgress;
            }
        }
    }

    pub fn fail_current(&mut self, reason: &str) {
        if self.current_index < self.tasks.len() {
            self.tasks[self.current_index].status = TaskStatus::Failed(reason.to_string());
        }
    }

    /// Dynamic Re-planning: inserts an urgent repair step immediately after the failed step
    pub fn insert_repair_step(&mut self, title: &str, reason: &str) {
        let insert_pos = if self.current_index < self.tasks.len() {
            self.current_index + 1
        } else {
            self.tasks.len()
        };

        let new_task = SubTask {
            id: 0, // reindexed below
            title: format!("[REPAIR] {}", title),
            description: reason.to_string(),
            status: TaskStatus::InProgress,
        };

        self.tasks.insert(insert_pos, new_task);
        self.current_index = insert_pos;

        // Reindex all tasks
        for (idx, task) in self.tasks.iter_mut().enumerate() {
            task.id = idx + 1;
        }
    }

    pub fn is_finished(&self) -> bool {
        self.tasks
            .iter()
            .all(|t| matches!(t.status, TaskStatus::Completed | TaskStatus::Skipped))
    }

    /// Renders a high-density ASCII progress summary for terminal display
    pub fn render_progress(&self) -> String {
        let mut out = format!("GOAL: {}\n", self.goal);
        let completed = self
            .tasks
            .iter()
            .filter(|t| matches!(t.status, TaskStatus::Completed))
            .count();
        out.push_str(&format!(
            "Progress: {}/{} steps completed\n\n",
            completed,
            self.tasks.len()
        ));

        for task in &self.tasks {
            let detail = match &task.status {
                TaskStatus::Failed(r) => format!(" (Error: {})", r),
                _ => String::new(),
            };
            out.push_str(&format!(
                "  {} {}. {}{}\n",
                task.status.badge(),
                task.id,
                task.title,
                detail
            ));
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_graph_lifecycle() {
        let mut graph = TaskGraph::new("Implement Auth Engine");
        graph.add_step("Scaffold jwt.rs", "Create JWT verification module");
        graph.add_step("Add jsonwebtoken crate", "Update Cargo dependencies");
        graph.add_step("Run verification test", "Verify unit test pass");

        assert_eq!(graph.tasks.len(), 3);
        assert_eq!(graph.current_index, 0);

        graph.start_current();
        assert_eq!(graph.tasks[0].status, TaskStatus::InProgress);

        graph.complete_current();
        assert_eq!(graph.tasks[0].status, TaskStatus::Completed);
        assert_eq!(graph.tasks[1].status, TaskStatus::InProgress);
        assert_eq!(graph.current_index, 1);

        // Simulate failure and dynamic repair insertion
        graph.fail_current("Missing OpenSSL headers");
        assert!(matches!(graph.tasks[1].status, TaskStatus::Failed(_)));

        graph.insert_repair_step(
            "Install OpenSSL or switch to rustls",
            "Compiler error: missing openssl",
        );
        assert_eq!(graph.tasks.len(), 4);
        assert_eq!(graph.current_index, 2);
        assert_eq!(
            graph.tasks[2].title,
            "[REPAIR] Install OpenSSL or switch to rustls"
        );
    }
}
