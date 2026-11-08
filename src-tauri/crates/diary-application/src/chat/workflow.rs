//! Chat workflow policy shared by transport adapters; no HTTP or runtime types.
use super::*;
use crate::agent::{
    AgentKind, AgentRequest, AgentRunner, Capability, Mode, PreparedAgentRun, ProfileDirectory,
    ProfileStatus, PromptSelection,
};
use crate::{Coordinator, Job, JobState};
use std::hash::{Hash, Hasher};

#[derive(Debug)]
pub enum Error {
    NotFound,
    ProfileNotReady,
    Packet(AgentError),
    Preparation { kind: AgentKind, error: AgentError },
    Coordination(crate::Error),
}

pub fn open(
    profiles: &dyn ProfileDirectory,
    alias: &str,
    locale: Locale,
    id: String,
    session: String,
) -> Result<Chat, Error> {
    let view = profiles.check(alias).ok_or(Error::NotFound)?;
    if view.status != Some(ProfileStatus::Ready) || view.chat != Capability::Ready {
        return Err(Error::ProfileNotReady);
    }
    Ok(Chat::new(
        id,
        session,
        ProfileHandle {
            alias: view.alias,
            kind: view.kind,
        },
        locale,
    ))
}
pub fn replace(chats: &mut Option<Chat>, chat: Chat) {
    if let Some(old) = chats.replace(chat) {
        old.cancel();
    }
}
pub fn close(chats: &mut Option<Chat>, session: &str, chat_id: &str) {
    if chats
        .as_ref()
        .is_some_and(|c| c.id == chat_id && c.session == session)
        && let Some(old) = chats.take()
    {
        old.cancel();
    }
}

pub struct TurnRequest {
    pub chat_id: String,
    pub request_id: String,
    pub message: String,
    pub document: Option<ChatDocument>,
}
impl TurnRequest {
    /// Idempotency only: preserve exact metadata framing and the existing prefixed std hash.
    pub fn fingerprint(&self) -> String {
        let doc_text = self.document.as_ref().map_or("", |d| d.body.as_str());
        let doc_meta = self.document.as_ref().map_or(String::new(), |d| {
            let kind = match d.kind {
                crate::agent::DocumentKind::Authentic => "authentic",
                crate::agent::DocumentKind::Source => "source",
            };
            format!("{}|{}|{}", kind, d.date, d.unsaved)
        });
        let parts = [&*self.chat_id, &*self.message, &*doc_meta, doc_text];
        (0u8..4)
            .map(|k| {
                let mut h = std::collections::hash_map::DefaultHasher::new();
                k.hash(&mut h);
                parts.as_slice().hash(&mut h);
                format!("{:016x}", h.finish())
            })
            .collect()
    }
    pub fn request(&self, chats: &Option<Chat>, session: &str) -> Result<AgentRequest, Error> {
        let chat = chats
            .as_ref()
            .filter(|c| c.id == self.chat_id && c.session == session)
            .ok_or(Error::NotFound)?;
        let packet = chat
            .packet(&self.message, self.document.clone())
            .map_err(Error::Packet)?;
        Ok(AgentRequest::new(
            chat.profile.clone(),
            chat.locale.clone(),
            Mode::Chat(packet),
        ))
    }
    pub fn prepare(
        self,
        request: AgentRequest,
        runner: &dyn AgentRunner,
    ) -> Result<PreparedTurn, Error> {
        let kind = request.profile().kind;
        let definition = runner
            .resolve(PromptSelection::for_request(&request))
            .map_err(|error| Error::Preparation { kind, error })?;
        let prepared = definition
            .prepare(request)
            .map_err(|error| Error::Preparation { kind, error })?;
        let digest = self.fingerprint();
        let reserved =
            self.message.len() + self.document.as_ref().map_or(0, |d| d.body.len()) + 4096;
        Ok(PreparedTurn {
            input: self,
            prepared,
            digest,
            reserved,
        })
    }
}

pub struct PreparedTurn {
    input: TurnRequest,
    prepared: PreparedAgentRun,
    digest: String,
    reserved: usize,
}
pub struct StartedTurn {
    pub running: Job,
    pub prepared: PreparedAgentRun,
    pub run: Run,
    pub shared: Shared,
}
pub enum StartOutcome {
    Running(Box<StartedTurn>),
    /// Publish terminal acknowledgement if the chat disappeared during preparation/admission.
    Closed(Option<Job>),
}
impl PreparedTurn {
    pub fn admit(
        &self,
        coordinator: &mut Coordinator,
        session: &str,
        now: u64,
        job_id: &str,
    ) -> Result<Job, Error> {
        coordinator
            .admit_agent(
                session,
                now,
                &self.input.request_id,
                &self.digest,
                job_id,
                self.reserved,
            )
            .map_err(Error::Coordination)
    }
    /// Caller holds coordinator before chat; capture/preparation never holds these locks together.
    pub fn start(
        self,
        coordinator: &mut Coordinator,
        chats: &mut Option<Chat>,
        session: &str,
        now: u64,
        job: &Job,
    ) -> StartOutcome {
        let Some(chat) = chats.as_mut().filter(|c| c.id == self.input.chat_id) else {
            return StartOutcome::Closed(
                coordinator
                    .transition(session, now, &job.id, JobState::Cancelled)
                    .ok(),
            );
        };
        let run = Run::new();
        let shared = Shared::default();
        chat.start(
            &job.id,
            &self.input.message,
            Run::from(run.token()),
            shared.clone(),
            self.prepared.provenance().clone(),
        );
        chat.observe_admission(
            self.input.request_id,
            self.input.document.as_ref().map(|d| d.date.to_string()),
            now,
        );
        let running = coordinator
            .transition(session, now, &job.id, JobState::Running)
            .expect("new admission");
        StartOutcome::Running(Box::new(StartedTurn {
            running,
            prepared: self.prepared,
            run,
            shared,
        }))
    }
}

pub fn finish(
    chats: &mut Option<Chat>,
    job_id: &str,
    result: Result<AgentOutput, AgentError>,
    now: u64,
) -> JobState {
    let terminal = match &result {
        Ok(AgentOutput::Reply(_)) => JobState::Succeeded,
        Err(e) if e.0 == AgentErrorCode::Cancelled => JobState::Cancelled,
        _ => JobState::Failed,
    };
    if let Some(chat) = chats.as_mut() {
        chat.finish(job_id, result);
        chat.observe_completion(job_id, now);
    }
    terminal
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Limits, agent::*};
    struct Profiles;
    impl ProfileDirectory for Profiles {
        fn list(&self) -> Vec<ProfileView> {
            self.check("ready").into_iter().collect()
        }
        fn check(&self, alias: &str) -> Option<ProfileView> {
            (alias != "missing").then(|| ProfileView {
                alias: alias.into(),
                kind: AgentKind::Claude,
                status: Some(if alias == "blocked" {
                    ProfileStatus::Unavailable
                } else {
                    ProfileStatus::Ready
                }),
                reason: None,
                chat: Capability::Ready,
                generate: Capability::Ready,
            })
        }
    }
    struct Runner;
    impl PromptResolver for Runner {
        fn resolve(&self, selection: PromptSelection) -> Result<ResolvedPrompt, AgentError> {
            Ok(crate::agent::test_prompts::definition(selection))
        }
    }
    impl AgentRunner for Runner {
        fn check(&self, _: &ProfileHandle) -> ProfileCheck {
            ProfileStatus::Ready.into()
        }
        fn run_prepared(
            &self,
            _: PreparedAgentRun,
            _: &mut dyn EventSink,
            _: &CancelToken,
        ) -> Result<AgentOutput, AgentError> {
            Ok(AgentOutput::Reply("reply".into()))
        }
    }
    fn setup() -> (Coordinator, Option<Chat>) {
        let mut c = Coordinator::new("instance".into(), Limits::default());
        c.open("session", 0).unwrap();
        let chat = open(
            &Profiles,
            "ready",
            Locale::parse("ko-KR").unwrap(),
            "chat".into(),
            "session".into(),
        )
        .unwrap();
        (c, Some(chat))
    }
    fn prepared(chats: &Option<Chat>) -> PreparedTurn {
        let input = TurnRequest {
            chat_id: "chat".into(),
            request_id: "request".into(),
            message: "message".into(),
            document: None,
        };
        let request = input.request(chats, "session").unwrap();
        input.prepare(request, &Runner).unwrap()
    }
    #[test]
    fn profile_checks_and_session_ownership_are_transport_independent() {
        assert!(matches!(
            open(
                &Profiles,
                "missing",
                Locale::parse("en-US").unwrap(),
                "c".into(),
                "session".into()
            ),
            Err(Error::NotFound)
        ));
        assert!(matches!(
            open(
                &Profiles,
                "blocked",
                Locale::parse("en-US").unwrap(),
                "c".into(),
                "session".into()
            ),
            Err(Error::ProfileNotReady)
        ));
        let (_, chats) = setup();
        let input = TurnRequest {
            chat_id: "chat".into(),
            request_id: "request".into(),
            message: "message".into(),
            document: None,
        };
        assert!(matches!(
            input.request(&chats, "other-session"),
            Err(Error::NotFound)
        ));
        assert_eq!(chats.as_ref().unwrap().profile.kind, AgentKind::Claude);
    }
    #[test]
    fn retry_reuses_job_and_busy_cannot_be_bypassed_with_another_request() {
        let (mut c, mut chats) = setup();
        let first = prepared(&chats);
        let job = first.admit(&mut c, "session", 0, "job").unwrap();
        let StartOutcome::Running(started) = first.start(&mut c, &mut chats, "session", 0, &job)
        else {
            panic!("chat remains open")
        };
        assert_eq!(
            prepared(&chats)
                .admit(&mut c, "session", 0, "retry")
                .unwrap()
                .id,
            "job"
        );
        assert_eq!(
            c.admit_agent(
                "session",
                0,
                "other-request",
                &"b".repeat(64),
                "other-job",
                0
            ),
            Err(crate::Error::Busy)
        );
        let result = crate::run::execute(
            &Runner,
            started.prepared,
            &started.run,
            &started.shared,
            &|| 1,
            &mut || {},
        );
        let state = finish(&mut chats, &job.id, result, 1);
        c.transition("session", 1, &job.id, state).unwrap();
        assert_eq!(state, JobState::Succeeded);
        assert_eq!(chats.as_ref().unwrap().transcript().len(), 2);
        let retry = prepared(&chats)
            .admit(&mut c, "session", 1, "retry-again")
            .unwrap();
        assert_eq!(retry.id, "job");
        assert_eq!(retry.state, JobState::Succeeded);
    }
    #[test]
    fn close_during_preparation_cancels_admission_without_starting_a_turn() {
        let (mut c, mut chats) = setup();
        let turn = prepared(&chats);
        close(&mut chats, "other-session", "chat");
        assert!(chats.is_some());
        close(&mut chats, "session", "chat");
        let job = turn.admit(&mut c, "session", 0, "job").unwrap();
        let StartOutcome::Closed(Some(cancelled)) =
            turn.start(&mut c, &mut chats, "session", 0, &job)
        else {
            panic!("closed chat must not start")
        };
        assert_eq!(cancelled.state, JobState::Cancelled);
        assert!(chats.is_none());
        assert!(c.active_agent("session", 0).unwrap().is_none());
    }
    #[test]
    fn cancellation_and_session_close_preserve_transcript_and_inference_gate() {
        let (mut c, mut chats) = setup();
        let turn = prepared(&chats);
        let job = turn.admit(&mut c, "session", 0, "job").unwrap();
        let StartOutcome::Running(started) = turn.start(&mut c, &mut chats, "session", 0, &job)
        else {
            panic!("chat remains open")
        };
        assert!(chats.as_ref().unwrap().cancel_job(&job.id));
        let result = crate::run::execute(
            &Runner,
            started.prepared,
            &started.run,
            &started.shared,
            &|| 0,
            &mut || {},
        );
        let state = finish(&mut chats, &job.id, result, 0);
        c.transition("session", 0, &job.id, state).unwrap();
        assert_eq!(state, JobState::Cancelled);
        assert!(chats.as_ref().unwrap().transcript().is_empty());
        c.begin_close("session", 0).unwrap();
        let mut next = prepared(&chats);
        next.input.request_id = "next-request".into();
        assert!(matches!(
            next.admit(&mut c, "session", 0, "next-job"),
            Err(Error::Coordination(crate::Error::Busy))
        ));
    }
}
