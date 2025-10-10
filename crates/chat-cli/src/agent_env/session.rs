use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

use super::worker::Worker;
use super::worker_job::WorkerJob;
use super::worker_task::WorkerTask;
use super::model_providers::ModelProvider;
use super::worker_tasks::{AgentLoop, AgentLoopInput};

/// Maximum number of inactive jobs to keep in memory
pub const MAX_INACTIVE_JOBS: usize = 3;

pub struct Session {
    model_providers: Vec<Arc<dyn ModelProvider>>,
    workers: Arc<Mutex<Vec<Arc<Worker>>>>,
    jobs: Arc<Mutex<Vec<Arc<WorkerJob>>>>,
}

impl Session {
    pub fn new(model_providers: Vec<Arc<dyn ModelProvider>>) -> Self {
        Self {
            model_providers,
            workers: Arc::new(Mutex::new(Vec::new())),
            jobs: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn build_worker(&self, name: String) -> Arc<Worker> {
        let model_provider = self.model_providers.first()
            .expect("At least one model provider required")
            .clone();
        
        let worker = Arc::new(Worker::new(
            name,
            model_provider,
        ));
        
        self.workers.lock().unwrap().push(worker.clone());
        worker
    }

    pub fn run_agent_loop(
        &self,
        _worker: Arc<Worker>,
        _input: AgentLoopInput,
    ) -> Result<Arc<WorkerJob>, eyre::Error> {
        unimplemented!("run_agent_loop will be reimplemented in EventBus architecture")
    }

    pub fn cancel_all_jobs(&self) {
        let jobs = self.jobs.lock().unwrap();
        for job in jobs.iter() {
            job.cancel();
        }
    }

    /// Cleanup old inactive jobs, keeping only MAX_INACTIVE_JOBS most recent
    pub fn cleanup_inactive_jobs(&self) {
        let mut jobs = self.jobs.lock().unwrap();
        
        // Separate active and inactive jobs
        let (active, mut inactive): (Vec<_>, Vec<_>) = jobs
            .iter()
            .cloned()
            .partition(|job| job.is_active());
        
        // Keep only last MAX_INACTIVE_JOBS inactive jobs
        if inactive.len() > MAX_INACTIVE_JOBS {
            let keep_from = inactive.len() - MAX_INACTIVE_JOBS;
            inactive.drain(0..keep_from);
        }
        
        // Rebuild jobs list: active + recent inactive
        *jobs = active;
        jobs.extend(inactive);
    }

    /// Get count of active and inactive jobs
    pub fn get_job_counts(&self) -> (usize, usize) {
        let jobs = self.jobs.lock().unwrap();
        let active = jobs.iter().filter(|j| j.is_active()).count();
        let inactive = jobs.len() - active;
        (active, inactive)
    }

    /// Wait for all active jobs to complete
    pub async fn wait_for_all_jobs(&self) {
        loop {
            let has_active = {
                let jobs = self.jobs.lock().unwrap();
                jobs.iter().any(|job| job.is_active())
            };
            
            if !has_active {
                break;
            }
            
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}
