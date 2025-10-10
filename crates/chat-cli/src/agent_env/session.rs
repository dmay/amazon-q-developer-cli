use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::worker::Worker;
use super::worker_job::WorkerJob;
use super::worker_task::WorkerTask;
use super::model_providers::ModelProvider;
use super::worker_tasks::{AgentLoop, AgentLoopInput};
use super::event_bus::EventBus;
use super::events::{AgentEnvironmentEvent, WorkerEvent, WorkerLifecycleState};

/// Maximum number of inactive jobs to keep in memory
pub const MAX_INACTIVE_JOBS: usize = 3;

pub struct Session {
    event_bus: EventBus,
    model_providers: Vec<Arc<dyn ModelProvider>>,
    workers: Arc<Mutex<Vec<Arc<Worker>>>>,
    jobs: Arc<Mutex<Vec<Arc<WorkerJob>>>>,
}

impl Session {
    pub fn new(event_bus: EventBus, model_providers: Vec<Arc<dyn ModelProvider>>) -> Self {
        Self {
            event_bus,
            model_providers,
            workers: Arc::new(Mutex::new(Vec::new())),
            jobs: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn event_bus(&self) -> &EventBus {
        &self.event_bus
    }

    pub fn build_worker(&self, name: String) -> Arc<Worker> {
        let model_provider = self.model_providers.first()
            .expect("At least one model provider required")
            .clone();
        
        let worker = Arc::new(Worker::new(
            name.clone(),
            model_provider,
        ));
        
        self.workers.lock().unwrap().push(worker.clone());
        
        // Publish WorkerEvent::Created
        self.event_bus.publish(AgentEnvironmentEvent::Worker(
            WorkerEvent::Created {
                worker_id: worker.id,
                name,
                timestamp: Instant::now(),
            }
        ));
        
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
    
    /// Set worker lifecycle state and publish event
    pub fn set_worker_lifecycle_state(
        &self,
        worker_id: Uuid,
        new_state: WorkerLifecycleState,
    ) {
        // Find worker
        let workers = self.workers.lock().unwrap();
        if let Some(worker) = workers.iter().find(|w| w.id == worker_id) {
            // Get old state
            let old_state = {
                let state = worker.lifecycle_state.lock().unwrap();
                *state
            };
            
            // Update state
            {
                let mut state = worker.lifecycle_state.lock().unwrap();
                *state = new_state;
            }
            
            // Publish event
            self.event_bus.publish(AgentEnvironmentEvent::Worker(
                WorkerEvent::LifecycleStateChanged {
                    worker_id,
                    old_state,
                    new_state,
                    timestamp: Instant::now(),
                }
            ));
        }
    }
    
    /// Delete worker and publish event
    pub fn delete_worker(&self, worker_id: Uuid) -> Result<(), eyre::Error> {
        // Cancel any active jobs for this worker
        self.cancel_worker_jobs(worker_id)?;
        
        // Remove worker
        let mut workers = self.workers.lock().unwrap();
        workers.retain(|w| w.id != worker_id);
        
        // Publish event
        self.event_bus.publish(AgentEnvironmentEvent::Worker(
            WorkerEvent::Deleted {
                worker_id,
                timestamp: Instant::now(),
            }
        ));
        
        Ok(())
    }
    
    /// Get worker by ID
    pub fn get_worker(&self, worker_id: Uuid) -> Option<Arc<Worker>> {
        let workers = self.workers.lock().unwrap();
        workers.iter().find(|w| w.id == worker_id).cloned()
    }
    
    /// Cancel all jobs for a specific worker
    pub fn cancel_worker_jobs(&self, worker_id: Uuid) -> Result<(), eyre::Error> {
        let jobs = self.jobs.lock().unwrap();
        for job in jobs.iter() {
            if job.worker.id == worker_id {
                job.cancel();
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_env::model_providers::{ModelProvider, ModelRequest, ModelResponse, ModelResponseChunk};
    use async_trait::async_trait;
    use eyre::Result;
    
    // Mock model provider for testing
    struct MockModelProvider;
    
    #[async_trait]
    impl ModelProvider for MockModelProvider {
        async fn request(
            &self,
            _request: ModelRequest,
            _when_receiving_begin: Box<dyn Fn() + Send>,
            _when_received: Box<dyn Fn(ModelResponseChunk) + Send>,
            _cancellation_token: CancellationToken,
        ) -> Result<ModelResponse> {
            Ok(ModelResponse {
                content: "mock response".to_string(),
                tool_requests: vec![],
            })
        }
    }
    
    fn create_test_session() -> Session {
        let event_bus = EventBus::default();
        let model_providers: Vec<Arc<dyn ModelProvider>> = vec![Arc::new(MockModelProvider)];
        Session::new(event_bus, model_providers)
    }
    
    #[test]
    fn test_worker_creation_publishes_event() {
        let session = create_test_session();
        let mut receiver = session.event_bus().subscribe();
        
        // Create worker
        let worker = session.build_worker("test_worker".to_string());
        
        // Verify event was published
        let event = receiver.try_recv().expect("Expected WorkerEvent::Created");
        match event {
            AgentEnvironmentEvent::Worker(WorkerEvent::Created { worker_id, name, .. }) => {
                assert_eq!(worker_id, worker.id);
                assert_eq!(name, "test_worker");
            }
            _ => panic!("Expected WorkerEvent::Created"),
        }
    }
    
    #[test]
    fn test_worker_deletion_publishes_event() {
        let session = create_test_session();
        let worker = session.build_worker("test_worker".to_string());
        let worker_id = worker.id;
        
        // Subscribe after creation to avoid getting Created event
        let mut receiver = session.event_bus().subscribe();
        
        // Delete worker
        session.delete_worker(worker_id).expect("Failed to delete worker");
        
        // Verify event was published
        let event = receiver.try_recv().expect("Expected WorkerEvent::Deleted");
        match event {
            AgentEnvironmentEvent::Worker(WorkerEvent::Deleted { worker_id: deleted_id, .. }) => {
                assert_eq!(deleted_id, worker_id);
            }
            _ => panic!("Expected WorkerEvent::Deleted"),
        }
        
        // Verify worker was removed
        assert!(session.get_worker(worker_id).is_none());
    }
    
    #[test]
    fn test_lifecycle_state_transitions() {
        let session = create_test_session();
        let worker = session.build_worker("test_worker".to_string());
        let worker_id = worker.id;
        
        // Subscribe after creation
        let mut receiver = session.event_bus().subscribe();
        
        // Transition to Busy
        session.set_worker_lifecycle_state(worker_id, WorkerLifecycleState::Busy);
        
        // Verify event was published
        let event = receiver.try_recv().expect("Expected LifecycleStateChanged");
        match event {
            AgentEnvironmentEvent::Worker(WorkerEvent::LifecycleStateChanged { 
                worker_id: wid, 
                old_state, 
                new_state, 
                .. 
            }) => {
                assert_eq!(wid, worker_id);
                assert_eq!(old_state, WorkerLifecycleState::Idle);
                assert_eq!(new_state, WorkerLifecycleState::Busy);
            }
            _ => panic!("Expected WorkerEvent::LifecycleStateChanged"),
        }
        
        // Verify state was updated in worker
        assert_eq!(*worker.lifecycle_state.lock().unwrap(), WorkerLifecycleState::Busy);
        
        // Transition to IdleFailed
        session.set_worker_lifecycle_state(worker_id, WorkerLifecycleState::IdleFailed);
        
        let event = receiver.try_recv().expect("Expected LifecycleStateChanged");
        match event {
            AgentEnvironmentEvent::Worker(WorkerEvent::LifecycleStateChanged { 
                old_state, 
                new_state, 
                .. 
            }) => {
                assert_eq!(old_state, WorkerLifecycleState::Busy);
                assert_eq!(new_state, WorkerLifecycleState::IdleFailed);
            }
            _ => panic!("Expected WorkerEvent::LifecycleStateChanged"),
        }
    }
    
    #[test]
    fn test_multiple_workers_dont_interfere() {
        let session = create_test_session();
        let mut receiver = session.event_bus().subscribe();
        
        // Create two workers
        let worker1 = session.build_worker("worker1".to_string());
        let worker2 = session.build_worker("worker2".to_string());
        
        // Verify both Created events
        let event1 = receiver.try_recv().expect("Expected first Created event");
        let event2 = receiver.try_recv().expect("Expected second Created event");
        
        // Verify worker_ids are different
        let id1 = event1.worker_id().expect("Expected worker_id");
        let id2 = event2.worker_id().expect("Expected worker_id");
        assert_ne!(id1, id2);
        
        // Change state of worker1
        session.set_worker_lifecycle_state(worker1.id, WorkerLifecycleState::Busy);
        
        // Verify only worker1's state changed
        assert_eq!(*worker1.lifecycle_state.lock().unwrap(), WorkerLifecycleState::Busy);
        assert_eq!(*worker2.lifecycle_state.lock().unwrap(), WorkerLifecycleState::Idle);
    }
}
