//! Native harness composition only: deterministic in-memory jobs through application ports.
use diary_application::{Coordinator, Error, JobState};
pub fn admit(coordinator: &mut Coordinator, session: &str, now: u64) -> Result<(), Error> {
    let job = coordinator.admit(
        session,
        now,
        "scaffold-fake-request",
        &"a".repeat(64),
        "scaffold-fake-job",
        0,
    )?;
    // Reopening the same lease returns the admitted job, including a terminal state.
    if job.state != JobState::Queued {
        return Ok(());
    }
    coordinator.publish(session, now, &job.id, 512)?;
    coordinator.transition(session, now, &job.id, JobState::Running)?;
    coordinator.publish(session, now, &job.id, 512)?;
    Ok(())
}

#[derive(serde::Serialize)]
pub struct FolderCheck {
    pub alias: String,
    pub outcome: String,
}
