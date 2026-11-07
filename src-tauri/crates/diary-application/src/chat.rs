//! One memory-only chat: fixed profile and locale, a server-held transcript and at most one turn.
//!
//! Nothing here touches disk or logs. A turn adds its user and assistant messages together and
//! only on success, so a failed or cancelled turn leaves the transcript as the client saw it.
use crate::agent::{
    AgentError, AgentErrorCode, AgentOutput, ChatDocument, ChatPacket, Locale, ProfileHandle, Role,
    Run, Turn,
};
use crate::run::Shared;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Running,
    Done,
    Failed(AgentErrorCode),
}

pub struct ActiveTurn {
    pub job_id: String,
    pub run: Run,
    pub shared: Shared,
    pub outcome: Outcome,
    pub request_id: Option<String>,
    pub origin_date: Option<String>,
    pub started_at: u64,
    pub finished_at: Option<u64>,
    message: String,
}

pub struct Chat {
    pub id: String,
    pub session: String,
    pub profile: ProfileHandle,
    pub locale: Locale,
    transcript: Vec<Turn>,
    turn: Option<ActiveTurn>,
}

impl Chat {
    pub fn new(id: String, session: String, profile: ProfileHandle, locale: Locale) -> Self {
        Self {
            id,
            session,
            profile,
            locale,
            transcript: Vec::new(),
            turn: None,
        }
    }
    pub fn transcript(&self) -> &[Turn] {
        &self.transcript
    }
    pub fn turn(&self) -> Option<&ActiveTurn> {
        self.turn.as_ref()
    }

    /// The packet for the next message. Over any bound this is `context`; nothing is truncated.
    pub fn packet(
        &self,
        message: &str,
        document: Option<ChatDocument>,
    ) -> Result<ChatPacket, AgentError> {
        ChatPacket::new(document, self.transcript.clone(), message.to_owned())
    }

    /// Records an admitted turn. The caller has already passed the busy gate.
    pub fn start(&mut self, job_id: &str, message: &str, run: Run, shared: Shared) {
        self.turn = Some(ActiveTurn {
            job_id: job_id.into(),
            run,
            shared,
            outcome: Outcome::Running,
            request_id: None,
            origin_date: None,
            started_at: 0,
            finished_at: None,
            message: message.into(),
        });
    }

    /// Capture body-less admission metadata while the caller holds the chat lock.
    pub fn observe_admission(&mut self, request_id: String, origin_date: Option<String>, now: u64) {
        if let Some(turn) = self.turn.as_mut() {
            turn.request_id = Some(request_id);
            turn.origin_date = origin_date;
            turn.started_at = now;
        }
    }

    pub fn observe_completion(&mut self, job_id: &str, now: u64) {
        if let Some(turn) = self.turn.as_mut().filter(|t| t.job_id == job_id) {
            turn.finished_at.get_or_insert(now);
        }
    }

    /// Applies the run result. Returns false when `job_id` is no longer the active turn.
    pub fn finish(&mut self, job_id: &str, result: Result<AgentOutput, AgentError>) -> bool {
        let Some(turn) = self.turn.as_mut().filter(|t| t.job_id == job_id) else {
            return false;
        };
        turn.outcome = match result {
            Ok(AgentOutput::Reply(text)) => {
                let user = std::mem::take(&mut turn.message);
                self.transcript.push(Turn {
                    role: Role::User,
                    text: user,
                });
                self.transcript.push(Turn {
                    role: Role::Assistant,
                    text,
                });
                Outcome::Done
            }
            // A chat run can only produce a reply.
            Ok(_) => Outcome::Failed(AgentErrorCode::InvalidOutput),
            Err(e) => Outcome::Failed(e.0),
        };
        true
    }

    /// Cancels the active turn if it is `job_id`. True only for the first cancel.
    pub fn cancel_job(&self, job_id: &str) -> bool {
        self.turn
            .as_ref()
            .filter(|t| t.job_id == job_id)
            .is_some_and(|t| t.run.request_cancel())
    }
    /// Cancels whatever is running; used when the chat is closed or its lease ends.
    pub fn cancel(&self) {
        if let Some(t) = &self.turn {
            t.run.request_cancel();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{AgentKind, DocumentKind, MAX_USER_MESSAGE_BYTES};
    use diary_domain::authentic::DiaryDate;

    fn chat() -> Chat {
        Chat::new(
            "c".into(),
            "s".into(),
            ProfileHandle {
                alias: "p".into(),
                kind: AgentKind::Codex,
            },
            Locale::parse("en-US").unwrap(),
        )
    }
    fn run(c: &mut Chat, job: &str, msg: &str, r: Result<AgentOutput, AgentError>) {
        c.start(job, msg, Run::new(), Default::default());
        assert!(c.finish(job, r));
    }

    #[test]
    fn success_appends_both_turns_failure_and_cancel_append_none() {
        let mut c = chat();
        run(&mut c, "j1", "hi", Ok(AgentOutput::Reply("hello".into())));
        assert_eq!(c.transcript().len(), 2);
        run(&mut c, "j2", "again", Err(AgentErrorCode::Failed.into()));
        run(&mut c, "j3", "again", Err(AgentErrorCode::Cancelled.into()));
        assert_eq!(c.transcript().len(), 2);
        assert_eq!(
            c.turn().unwrap().outcome,
            Outcome::Failed(AgentErrorCode::Cancelled)
        );
    }

    #[test]
    fn late_result_for_a_replaced_turn_is_ignored() {
        let mut c = chat();
        c.start("j1", "a", Run::new(), Default::default());
        c.start("j2", "b", Run::new(), Default::default());
        assert!(!c.finish("j1", Ok(AgentOutput::Reply("x".into()))));
        assert!(c.transcript().is_empty());
    }

    #[test]
    fn chat_bounds_fail_without_truncation() {
        let mut c = chat();
        let big = "a".repeat(MAX_USER_MESSAGE_BYTES + 1);
        assert_eq!(c.packet(&big, None).unwrap_err().0, AgentErrorCode::Context);
        for i in 0..51 {
            run(
                &mut c,
                &format!("j{i}"),
                "q",
                Ok(AgentOutput::Reply("a".into())),
            );
        }
        // More than 100 turns held: no further message can be sent, and nothing is dropped.
        assert_eq!(c.packet("q", None).unwrap_err().0, AgentErrorCode::Context);
        assert_eq!(c.transcript().len(), 102);
    }

    #[test]
    fn no_document_sends_no_context() {
        let c = chat();
        assert!(c.packet("q", None).unwrap().document().is_none());
        let d = ChatDocument {
            kind: DocumentKind::Authentic,
            date: DiaryDate::from_ymd(2026, 11, 6).unwrap(),
            unsaved: true,
            body: "b".into(),
        };
        assert!(c.packet("q", Some(d)).unwrap().document().unwrap().unsaved);
    }

    #[test]
    fn cancel_commits_once() {
        let mut c = chat();
        c.start("j1", "a", Run::new(), Default::default());
        assert!(c.cancel_job("j1"));
        assert!(!c.cancel_job("j1"));
        assert!(!c.cancel_job("other"));
    }
}
