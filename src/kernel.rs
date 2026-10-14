use crate::compiler::{ExecutionPlan, PlanTask};
use anyhow::Result;
use colored::Colorize;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed { cached: bool, output_path: Option<PathBuf> },
    Failed(String),
}

pub struct KernelEngine {
    cache_dir: PathBuf,
}

impl KernelEngine {
    pub fn new<P: AsRef<Path>>(project_root: P) -> Self {
        let cache_dir = project_root.as_ref().join(".hypit").join("cache");
        std::fs::create_dir_all(&cache_dir).ok();
        Self { cache_dir }
    }

    pub fn compute_task_hash(task: &PlanTask) -> String {
        let mut hasher = Sha256::new();
        hasher.update(format!("{:?}", task.task_type).as_bytes());
        for (k, v) in &task.parameters {
            hasher.update(k.as_bytes());
            hasher.update(v.as_bytes());
        }
        for dep in &task.dependencies {
            hasher.update(dep.as_bytes());
        }
        hex::encode(hasher.finalize())
    }

    pub async fn execute<F>(&self, plan: &ExecutionPlan, mut step_callback: F) -> Result<()>
    where
        F: FnMut(&PlanTask, &TaskStatus),
    {
        let mut statuses: HashMap<String, TaskStatus> = plan
            .tasks
            .iter()
            .map(|t| (t.id.clone(), TaskStatus::Pending))
            .collect();

        let mut completed_ids = HashSet::new();
        let total_tasks = plan.tasks.len();
        let mut finished_count = 0;

        while finished_count < total_tasks {
            // Find all pending tasks whose dependencies are met
            let ready_tasks: Vec<PlanTask> = plan
                .tasks
                .iter()
                .filter(|t| {
                    if statuses.get(&t.id) != Some(&TaskStatus::Pending) {
                        return false;
                    }
                    t.dependencies.iter().all(|dep| completed_ids.contains(dep))
                })
                .cloned()
                .collect();

            if ready_tasks.is_empty() && finished_count < total_tasks {
                anyhow::bail!("Deadlock detected or cyclic dependency in execution graph");
            }

            for task in ready_tasks {
                let task_hash = Self::compute_task_hash(&task);
                let cached_file = self.cache_dir.join(format!("{}.artifact", task_hash));

                statuses.insert(task.id.clone(), TaskStatus::Running);
                step_callback(&task, &TaskStatus::Running);

                // Check cache hit
                let status = if cached_file.exists() {
                    TaskStatus::Completed {
                        cached: true,
                        output_path: Some(cached_file),
                    }
                } else {
                    // Simulate task execution / write artifact
                    std::fs::write(&cached_file, format!("artifact:{}:{}", task.id, task_hash)).ok();
                    TaskStatus::Completed {
                        cached: false,
                        output_path: Some(cached_file),
                    }
                };

                step_callback(&task, &status);
                statuses.insert(task.id.clone(), status);
                completed_ids.insert(task.id.clone());
                finished_count += 1;
            }
        }

        println!("{}", format!("✓ Pipeline completed successfully: {} tasks executed.", total_tasks).green().bold());
        Ok(())
    }
}
