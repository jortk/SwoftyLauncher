//! Scheduling for Addon Browser install jobs.
//!
//! The scheduler runs on the UI thread. It does not perform downloads or guarantee
//! installation safety: callers must also coordinate requirements discovered after
//! a job starts with the core installer.

use std::collections::{BTreeSet, VecDeque};

const MIN_LIMIT: usize = 1;
const MAX_LIMIT: usize = 10;
pub(crate) const DEFAULT_LIMIT: usize = 2;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum State {
    Queued,
    Active,
    Completed,
    Failed(String),
}

#[derive(Clone, Debug)]
pub(crate) struct Job<T> {
    pub(crate) id: String,
    pub(crate) request: T,
    /// Root addon and all requirements known before starting.
    pub(crate) claims: BTreeSet<String>,
    pub(crate) state: State,
}

pub(crate) struct Queue<T> {
    jobs: VecDeque<Job<T>>,
    limit: usize,
}

impl<T> Default for Queue<T> {
    fn default() -> Self {
        Self {
            jobs: VecDeque::new(),
            limit: DEFAULT_LIMIT,
        }
    }
}

impl<T> Queue<T> {
    pub(crate) fn limit(&self) -> usize {
        self.limit
    }

    pub(crate) fn set_limit(&mut self, limit: usize) {
        self.limit = limit.clamp(MIN_LIMIT, MAX_LIMIT);
    }

    pub(crate) fn jobs(&self) -> &VecDeque<Job<T>> {
        &self.jobs
    }

    pub(crate) fn pending(&self) -> usize {
        self.jobs
            .iter()
            .filter(|j| matches!(j.state, State::Queued | State::Active))
            .count()
    }

    /// Returns false when this addon is already queued or running.
    pub(crate) fn enqueue(&mut self, id: String, request: T, mut claims: BTreeSet<String>) -> bool {
        if self
            .jobs
            .iter()
            .any(|j| j.id == id && matches!(j.state, State::Queued | State::Active))
        {
            return false;
        }
        claims.insert(id.clone());
        self.jobs.push_back(Job {
            id,
            request,
            claims,
            state: State::Queued,
        });
        true
    }

    /// Record a request that could not be queued, without disturbing pending jobs.
    pub(crate) fn record_failure(&mut self, id: String, request: T, error: String) {
        self.jobs.push_back(Job {
            claims: BTreeSet::from([id.clone()]),
            id,
            request,
            state: State::Failed(error),
        });
    }

    /// Update an active or queued job's known requirements. A running job may
    /// discover more requirements after another job has already started; callers
    /// must still enforce safety in the installer itself.
    pub(crate) fn update_claims(&mut self, id: &str, mut claims: BTreeSet<String>) -> bool {
        let Some(job) = self
            .jobs
            .iter_mut()
            .find(|j| j.id == id && matches!(j.state, State::Queued | State::Active))
        else {
            return false;
        };
        claims.insert(id.to_owned());
        job.claims = claims;
        true
    }

    /// Start the earliest eligible job, allowing unrelated jobs past blocked ones.
    /// Return its addon ID, which remains valid when other rows are removed.
    pub(crate) fn start_next(&mut self) -> Option<String> {
        if self.jobs.iter().filter(|j| j.state == State::Active).count() >= self.limit {
            return None;
        }
        let index = self.jobs.iter().enumerate().find_map(|(index, candidate)| {
            if candidate.state != State::Queued {
                return None;
            }
            let blocked = self.jobs.iter().enumerate().any(|(other_index, other)| {
                (other.state == State::Active || (other_index < index && other.state == State::Queued))
                    && !other.claims.is_disjoint(&candidate.claims)
            });
            (!blocked).then_some(index)
        })?;
        self.jobs[index].state = State::Active;
        Some(self.jobs[index].id.clone())
    }

    pub(crate) fn finish(&mut self, id: &str, result: Result<(), String>) -> bool {
        let Some(job) = self
            .jobs
            .iter_mut()
            .find(|j| j.id == id && j.state == State::Active)
        else {
            return false;
        };
        job.state = match result {
            Ok(()) => State::Completed,
            Err(error) => State::Failed(error),
        };
        true
    }

    pub(crate) fn remove_queued(&mut self, id: &str) -> bool {
        let Some(index) = self
            .jobs
            .iter()
            .position(|j| j.id == id && j.state == State::Queued)
        else {
            return false;
        };
        self.jobs.remove(index);
        true
    }

    /// Retry a specific failed row, unless another request for its addon is pending.
    pub(crate) fn retry_at(&mut self, index: usize, id: &str) -> bool {
        let Some(job) = self
            .jobs
            .get(index)
            .filter(|job| job.id == id && matches!(job.state, State::Failed(_)))
        else {
            return false;
        };
        if self
            .jobs
            .iter()
            .any(|other| other.id == job.id && matches!(other.state, State::Queued | State::Active))
        {
            return false;
        }
        let mut job = self.jobs.remove(index).expect("job index came from the queue");
        job.state = State::Queued;
        self.jobs.push_back(job);
        true
    }

    pub(crate) fn clear_finished(&mut self) {
        self.jobs
            .retain(|j| matches!(j.state, State::Queued | State::Active));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add(q: &mut Queue<()>, id: &str, claims: &[&str]) -> bool {
        q.enqueue(id.into(), (), claims.iter().map(|s| (*s).into()).collect())
    }

    fn start(q: &mut Queue<()>) -> Option<String> {
        q.start_next()
    }

    #[test]
    fn fifo_respects_limit_and_skips_blocked_jobs() {
        let mut q = Queue::default();
        add(&mut q, "a", &["shared"]);
        add(&mut q, "b", &["shared"]);
        add(&mut q, "c", &[]);
        assert_eq!(start(&mut q).as_deref(), Some("a"));
        assert_eq!(start(&mut q).as_deref(), Some("c"));
        assert_eq!(start(&mut q), None);
        assert!(q.finish("a", Ok(())));
        assert_eq!(start(&mut q).as_deref(), Some("b"));
    }

    #[test]
    fn later_job_cannot_overtake_an_earlier_queued_shared_dependency() {
        let mut q = Queue::default();
        q.set_limit(3);
        add(&mut q, "a", &["x"]);
        add(&mut q, "b", &["x", "y"]);
        add(&mut q, "c", &["y"]);
        add(&mut q, "d", &[]);
        assert_eq!(start(&mut q).as_deref(), Some("a"));
        assert_eq!(start(&mut q).as_deref(), Some("d"));
        assert_eq!(start(&mut q), None);
        assert!(q.finish("a", Ok(())));
        assert_eq!(start(&mut q).as_deref(), Some("b"));
        assert_eq!(start(&mut q), None);
        assert!(q.finish("b", Ok(())));
        assert_eq!(start(&mut q).as_deref(), Some("c"));
    }

    #[test]
    fn changing_limit_does_not_interrupt_running_jobs() {
        let mut q = Queue::default();
        for id in ["a", "b", "c"] {
            add(&mut q, id, &[]);
        }
        assert_eq!(q.limit(), 2);
        start(&mut q);
        start(&mut q);
        q.set_limit(1);
        assert_eq!(start(&mut q), None);
        q.finish("a", Ok(()));
        assert_eq!(start(&mut q), None);
        q.set_limit(10);
        assert_eq!(start(&mut q).as_deref(), Some("c"));
        q.set_limit(100);
        assert_eq!(q.limit(), 10);
        q.set_limit(0);
        assert_eq!(q.limit(), 1);
    }

    #[test]
    fn deduplicate_retry_remove_and_clear() {
        let mut q = Queue::default();
        assert!(add(&mut q, "a", &[]));
        assert!(!add(&mut q, "a", &[]));
        assert!(add(&mut q, "b", &[]));
        assert_eq!(q.pending(), 2);
        assert!(q.remove_queued("b"));
        start(&mut q);
        assert!(!q.remove_queued("a"));
        assert!(q.finish("a", Err("failed".into())));
        assert!(q.retry_at(0, "a"));
        assert_eq!(q.jobs()[0].state, State::Queued);
        assert_eq!(q.jobs()[0].request, ());
        start(&mut q);
        assert!(q.finish("a", Ok(())));
        q.clear_finished();
        assert!(q.jobs().is_empty());
    }

    #[test]
    fn updated_claims_block_later_starts_and_can_be_refreshed() {
        let mut q = Queue::default();
        add(&mut q, "a", &[]);
        add(&mut q, "b", &["shared"]);
        assert_eq!(start(&mut q).as_deref(), Some("a"));
        assert!(q.update_claims("a", BTreeSet::from(["shared".into()])));
        assert_eq!(start(&mut q), None);
        assert!(q.update_claims("b", BTreeSet::new()));
        assert_eq!(start(&mut q).as_deref(), Some("b"));
        assert!(q.finish("a", Ok(())));
        assert!(!q.update_claims("a", BTreeSet::new()));
    }

    #[test]
    fn rejected_batch_request_is_visible_without_disturbing_existing_job() {
        let mut q = Queue::default();
        assert!(q.enqueue("a".into(), "1.0", BTreeSet::new()));
        q.record_failure("a".into(), "2.0", "another version is queued".into());
        assert_eq!(q.pending(), 1);
        assert_eq!(q.jobs()[0].state, State::Queued);
        assert_eq!(q.jobs()[1].request, "2.0");
        assert_eq!(
            q.jobs()[1].state,
            State::Failed("another version is queued".into())
        );
        assert!(!q.retry_at(1, "a"));
        assert_eq!(q.start_next().as_deref(), Some("a"));
        assert!(q.finish("a", Ok(())));
        assert!(q.retry_at(1, "a"));
        assert_eq!(q.jobs()[1].request, "2.0");
    }

    #[test]
    fn duplicate_request_does_not_replace_the_original_version() {
        let mut q = Queue::default();
        assert!(q.enqueue("a".into(), "1.0", BTreeSet::new()));
        assert!(!q.enqueue("a".into(), "2.0", BTreeSet::new()));
        assert_eq!(q.jobs()[0].request, "1.0");
        assert_eq!(q.start_next().as_deref(), Some("a"));
        assert!(!q.enqueue("a".into(), "2.0", BTreeSet::new()));
        assert_eq!(q.jobs()[0].request, "1.0");
    }

    #[test]
    fn completed_history_does_not_block_a_new_request_or_its_completion() {
        let mut q = Queue::default();
        assert!(add(&mut q, "a", &[]));
        assert_eq!(start(&mut q).as_deref(), Some("a"));
        assert!(q.finish("a", Ok(())));
        assert!(add(&mut q, "a", &[]));
        assert_eq!(start(&mut q).as_deref(), Some("a"));
        assert!(q.finish("a", Err("second attempt".into())));
        assert_eq!(q.jobs()[0].state, State::Completed);
        assert_eq!(q.jobs()[1].state, State::Failed("second attempt".into()));
    }

    #[test]
    fn retry_joins_the_back_and_cannot_duplicate_a_pending_request() {
        let mut q = Queue::default();
        add(&mut q, "a", &[]);
        start(&mut q);
        q.finish("a", Err("failed".into()));
        add(&mut q, "b", &[]);
        assert!(q.retry_at(0, "a"));
        assert!(!q.retry_at(1, "a"));
        assert_eq!(start(&mut q).as_deref(), Some("b"));
        assert_eq!(start(&mut q).as_deref(), Some("a"));
    }

    #[test]
    fn root_ids_are_automatically_claimed() {
        let mut q = Queue::default();
        add(&mut q, "dependency", &[]);
        add(&mut q, "root", &["dependency"]);
        assert_eq!(start(&mut q).as_deref(), Some("dependency"));
        assert_eq!(start(&mut q), None);
    }
}
