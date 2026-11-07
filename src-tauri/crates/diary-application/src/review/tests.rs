use super::*;
use crate::{
    authentic::{ConflictKind, CreateRequest, Page, RebuildReport, SaveOutcome, SaveRequest},
    operations::{Identity, Record},
};
use diary_domain::authentic::AuthenticHeader;
use std::{collections::HashMap, sync::Mutex as StdMutex};

struct Fnv;
impl Hasher for Fnv {
    fn sha256_hex(&self, bytes: &[u8]) -> String {
        let mut h: u64 = 0xcbf29ce484222325;
        for b in bytes {
            h = (h ^ *b as u64).wrapping_mul(0x100000001b3);
        }
        format!("{h:016x}")
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Fault {
    None,
    /// Commit the save, then fail the reply as a crash would.
    LostReply,
    /// Fail before anything is written.
    Reject,
}

struct Fake {
    file: StdMutex<Option<(u64, String)>>,
    ops: StdMutex<HashMap<String, Record>>,
    fault: StdMutex<Fault>,
    saves: StdMutex<u32>,
    /// Runs inside `save_review`, to probe serialization.
    probe: StdMutex<Option<Box<dyn Fn() + Send>>>,
}

fn date() -> DiaryDate {
    DiaryDate::parse("2026-11-07").unwrap()
}
fn rev(seq: u64, body: &str) -> AuthenticRevision {
    AuthenticRevision {
        sequence: seq,
        sha256: Fnv.sha256_hex(body.as_bytes()),
    }
}
fn op(n: u8) -> OperationId {
    OperationId::parse(&format!("00000000-0000-4000-8000-{n:012}")).unwrap()
}
fn header() -> AuthenticHeader {
    AuthenticHeader {
        id: "x".into(),
        date: date(),
        timezone: "UTC".into(),
        timezone_database: "x".into(),
        created_at: "x".into(),
        updated_at: "x".into(),
        revision: 1,
        extensions: Vec::new(),
    }
}

impl Fake {
    fn new(body: Option<&str>) -> Self {
        Self {
            file: StdMutex::new(body.map(|b| (1, b.to_owned()))),
            ops: Default::default(),
            fault: StdMutex::new(Fault::None),
            saves: StdMutex::new(0),
            probe: StdMutex::new(None),
        }
    }
    fn body(&self) -> Option<String> {
        self.file.lock().unwrap().as_ref().map(|f| f.1.clone())
    }
    fn external(&self, body: &str) {
        let mut f = self.file.lock().unwrap();
        let seq = f.as_ref().map_or(1, |f| f.0 + 1);
        *f = Some((seq, body.to_owned()));
    }
    fn current(&self) -> Option<AuthenticRevision> {
        self.file.lock().unwrap().as_ref().map(|(s, b)| rev(*s, b))
    }
}

impl AuthenticStore for Fake {
    fn load(&self, d: &DiaryDate) -> Result<Loaded, StoreError> {
        Ok(match self.file.lock().unwrap().as_ref() {
            None => Loaded::Virtual(*d),
            Some((s, b)) => Loaded::File {
                revision: rev(*s, b),
                header: Box::new(header()),
                body: b.clone(),
            },
        })
    }
    fn list(&self, _: Option<DiaryDate>, _: usize) -> Result<Page, StoreError> {
        unreachable!()
    }
    fn save(&self, _: SaveRequest) -> Result<SaveOutcome, StoreError> {
        unreachable!()
    }
    fn create_empty(&self, _: CreateRequest) -> Result<SaveOutcome, StoreError> {
        unreachable!()
    }
    fn refresh(&self) -> Result<RebuildReport, StoreError> {
        unreachable!()
    }
    fn operation(&self, id: &OperationId) -> Result<Option<Record>, StoreError> {
        Ok(self.ops.lock().unwrap().get(id.as_str()).cloned())
    }
    fn save_review(&self, r: ReviewSave) -> Result<SaveOutcome, StoreError> {
        if let Some(probe) = self.probe.lock().unwrap().take() {
            probe();
        }
        if let Some(rec) = self.ops.lock().unwrap().get(r.operation_id.as_str()) {
            let (head, _) = rec.result.as_deref().unwrap().split_once(' ').unwrap();
            let (s, d) = head.split_once(':').unwrap();
            return Ok(SaveOutcome {
                operation_id: r.operation_id,
                revision: AuthenticRevision {
                    sequence: s.parse().unwrap(),
                    sha256: d.into(),
                },
                buffer_generation: 0,
            });
        }
        let fault = *self.fault.lock().unwrap();
        if fault == Fault::Reject {
            *self.fault.lock().unwrap() = Fault::None;
            return Err(StoreError::Storage);
        }
        let mut file = self.file.lock().unwrap();
        if file.as_ref().map(|(s, b)| rev(*s, b)) != r.expected {
            return Err(StoreError::Conflict(ConflictKind::Stale));
        }
        let seq = file.as_ref().map_or(1, |f| f.0 + 1);
        let revision = rev(seq, &r.body);
        *file = Some((seq, r.body.clone()));
        *self.saves.lock().unwrap() += 1;
        self.ops.lock().unwrap().insert(
            r.operation_id.as_str().to_owned(),
            Record {
                id: r.operation_id.clone(),
                kind: KIND_REVIEW.into(),
                hash_version: 1,
                input_hash: String::new(),
                state: OperationState::Succeeded,
                checkpoint: 1,
                result: Some(format!(
                    "{seq}:{} {},{},{}",
                    revision.sha256, r.hunk, r.before, r.after
                )),
                identity: Identity {
                    workspace_id: String::new(),
                    root: String::new(),
                    native: String::new(),
                    adapter_version: 1,
                },
            },
        );
        if fault == Fault::LostReply {
            *self.fault.lock().unwrap() = Fault::None;
            return Err(StoreError::Uncertain);
        }
        Ok(SaveOutcome {
            operation_id: r.operation_id,
            revision,
            buffer_generation: 0,
        })
    }
}

const REVIEW: &str = "00000000-0000-4000-8000-0000000000aa";
const BASE: &str = "A\n\nB\n\nC\n";
const CAND: &str = "A\n\nB2\n\nX\n\nC\n";

fn start(store: &Fake, reviews: &Reviews) -> View {
    reviews
        .start(
            store,
            &Fnv,
            Start {
                session: "s",
                review_id: REVIEW,
                date: date(),
                candidate_id: "job-1",
                candidate_body: CAND,
                baseline: store.current(),
            },
        )
        .unwrap()
}

fn accept(store: &Fake, reviews: &Reviews, v: &View, n: u8, i: usize) -> Result<Decision, Error> {
    reviews.decide(
        store,
        &Fnv,
        "s",
        REVIEW,
        Decide {
            operation_id: op(n),
            hunk: v.pending[i].id.clone(),
            accept: true,
            expected: v.revision.clone(),
            set_digest: v.set_digest.clone(),
        },
    )
}

#[test]
fn t04_start_guards() {
    let store = Fake::new(Some(BASE));
    let reviews = Reviews::default();
    let stale = reviews.start(
        &store,
        &Fnv,
        Start {
            session: "s",
            review_id: REVIEW,
            date: date(),
            candidate_id: "job-1",
            candidate_body: CAND,
            baseline: Some(rev(9, "other")),
        },
    );
    assert_eq!(stale.unwrap_err(), Error::StaleBaseline);
    let v = start(&store, &reviews);
    assert_eq!(v.pending.len(), 2);
    assert_eq!(v.body, BASE);
    // An absent baseline is a virtual empty body and writes no file.
    let empty = Fake::new(None);
    let reviews = Reviews::default();
    let v = start(&empty, &reviews);
    assert_eq!((v.pending.len(), empty.body()), (4, None));
    assert_eq!(*empty.saves.lock().unwrap(), 0);
}

#[test]
fn t05_accept_is_durable_and_memory_follows() {
    let store = Fake::new(Some(BASE));
    let reviews = Reviews::default();
    let v = start(&store, &reviews);
    // A failed accept keeps the saved set; retrying with the same ID works.
    *store.fault.lock().unwrap() = Fault::Reject;
    assert_eq!(
        accept(&store, &reviews, &v, 1, 1).unwrap_err(),
        Error::Store(StoreError::Storage)
    );
    assert_eq!(store.body().unwrap(), BASE);
    let d = accept(&store, &reviews, &v, 1, 1).unwrap();
    let after = d.view.unwrap();
    assert_eq!(store.body().unwrap(), "A\n\nB\n\nX\n\nC\n");
    assert_eq!(after.body, store.body().unwrap());
    assert_eq!(after.pending.len(), 1);
    let receipt = d.receipt.unwrap();
    assert_eq!(receipt.before, v.set_digest);
    assert_eq!(receipt.after, after.set_digest);
    // Stale expectations are refused before any write.
    assert_eq!(
        accept(&store, &reviews, &v, 2, 0).unwrap_err(),
        Error::StaleRevision
    );
}

#[test]
fn t06_lost_reply_replay_and_restart() {
    let store = Fake::new(Some(BASE));
    let reviews = Reviews::default();
    let v = start(&store, &reviews);
    *store.fault.lock().unwrap() = Fault::LostReply;
    assert_eq!(
        accept(&store, &reviews, &v, 1, 1).unwrap_err(),
        Error::Uncertain
    );
    // Uncertain blocks other decisions and ending until reconciled.
    assert_eq!(
        accept(&store, &reviews, &v, 2, 0).unwrap_err(),
        Error::Uncertain
    );
    assert_eq!(reviews.end("s", REVIEW), Err(Error::Uncertain));
    // The original ID resolves it from the ledger: no second publish.
    let d = accept(&store, &reviews, &v, 1, 1).unwrap();
    assert_eq!(*store.saves.lock().unwrap(), 1);
    let now = d.view.unwrap();
    assert_eq!(now.pending.len(), 1);
    // A later accept, then replaying the first never regresses it.
    let d2 = accept(&store, &reviews, &now, 2, 0).unwrap();
    let both = d2.view.unwrap();
    assert!(both.pending.is_empty());
    let again = accept(&store, &reviews, &v, 1, 1).unwrap();
    assert_eq!(again.view.unwrap().body, both.body);
    assert_eq!(*store.saves.lock().unwrap(), 2);
    // After a restart the review is gone but the receipt is recoverable.
    let restarted = Reviews::default();
    let d = accept(&store, &restarted, &v, 1, 1).unwrap();
    assert!(d.view.is_none() && d.receipt.is_some());
    assert_eq!(store.body().unwrap(), CAND);
}

#[test]
fn t07_serialization_and_committed_hunks() {
    let store = std::sync::Arc::new(Fake::new(Some(BASE)));
    let reviews = std::sync::Arc::new(Reviews::default());
    let v = start(&store, &reviews);
    let (s2, r2, v2) = (store.clone(), reviews.clone(), v.clone());
    let seen = std::sync::Arc::new(StdMutex::new(Vec::new()));
    let seen2 = seen.clone();
    *store.probe.lock().unwrap() = Some(Box::new(move || {
        let second = accept(&s2, &r2, &v2, 7, 0);
        seen2
            .lock()
            .unwrap()
            .push((second.unwrap_err(), r2.end("s", REVIEW)));
    }));
    accept(&store, &reviews, &v, 1, 1).unwrap();
    assert_eq!(seen.lock().unwrap()[0], (Error::Busy, Err(Error::Busy)));
    // Rejection writes nothing; a decided hunk cannot be toggled.
    let now = reviews
        .decide(
            &*store,
            &Fnv,
            "s",
            REVIEW,
            Decide {
                operation_id: op(3),
                hunk: v.pending[0].id.clone(),
                accept: false,
                expected: Some(store.current().unwrap()),
                set_digest: review::set_digest(&Fnv, &BTreeSet::from([v.pending[1].id.clone()])),
            },
        )
        .unwrap();
    assert_eq!(*store.saves.lock().unwrap(), 1);
    assert!(now.view.unwrap().pending.is_empty());
    let toggled = accept(&store, &reviews, &v, 4, 0);
    assert!(matches!(
        toggled,
        Err(Error::StaleRevision | Error::SetMismatch | Error::Decided)
    ));
    assert_eq!(reviews.end("s", REVIEW), Ok(()));
}

#[test]
fn t08_external_change_invalidates_without_rebase() {
    let store = Fake::new(Some(BASE));
    let reviews = Reviews::default();
    let v = start(&store, &reviews);
    let d = accept(&store, &reviews, &v, 1, 1).unwrap().view.unwrap();
    store.external("A\n\nB\n\nX\n\nC\n\nlate\n");
    assert_eq!(
        accept(&store, &reviews, &d, 2, 0).unwrap_err(),
        Error::Invalidated
    );
    // Accepted text stays saved; the review stays invalid.
    assert!(store.body().unwrap().contains("late"));
    assert_eq!(
        accept(&store, &reviews, &d, 3, 0).unwrap_err(),
        Error::Invalidated
    );
    assert_eq!(reviews.end("s", REVIEW), Ok(()));
}

#[test]
fn guards_follow_the_candidate() {
    let store = Fake::new(Some(BASE));
    let reviews = Reviews::default();
    start(&store, &reviews);
    assert!(reviews.uses_job("s", "job"));
    assert!(!reviews.uses_job("s", "other"));
    assert!(!reviews.uses_job("t", "job"));
    assert!(reviews.has_date("s", &date()));
    reviews.clear();
    assert!(!reviews.uses_job("s", "job"));
}
