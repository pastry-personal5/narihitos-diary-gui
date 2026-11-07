//! Paragraph review sessions: a fixed baseline, a validated candidate and serialized accepts.
//!
//! Reviews are transient and session-owned. An accept is one durable `review_accept` save through
//! the Authentic store; memory changes only after the store acknowledges it, and a lost reply is
//! resolved by the original operation ID (also after a restart, when the review is gone).
use crate::{
    authentic::{AuthenticStore, Loaded, ReviewSave, StoreError},
    operations::{OperationId, OperationState},
};
use diary_domain::{
    authentic::{AuthenticRevision, DiaryDate},
    review::{self, Hasher, Hunk, Pending, ReviewError},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Mutex,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    NotFound,
    /// Another decision is in flight, or the date already has a review.
    Busy,
    /// An earlier accept's outcome is unknown; retry its operation ID.
    Uncertain,
    /// The saved entry changed outside this review; remaining suggestions are dropped.
    Invalidated,
    /// The saved entry is no longer the candidate's baseline; regenerate.
    StaleBaseline,
    StaleRevision,
    SetMismatch,
    /// The hunk is already accepted or rejected.
    Decided,
    Unsupported,
    Limit,
    Failed(String),
    Store(StoreError),
}
impl From<ReviewError> for Error {
    fn from(e: ReviewError) -> Self {
        match e {
            ReviewError::Unsupported => Self::Unsupported,
            ReviewError::Limit => Self::Limit,
        }
    }
}

pub const KIND_REVIEW: &str = "review_accept";

struct Review {
    session: String,
    date: DiaryDate,
    candidate_id: String,
    base: String,
    cand: String,
    candidate_digest: String,
    hunks: Vec<Hunk>,
    accepted: BTreeSet<String>,
    rejected: BTreeSet<String>,
    /// The saved revision this review expects; advances with its own accepts.
    revision: Option<AuthenticRevision>,
    busy: Option<OperationId>,
    uncertain: Option<OperationId>,
    invalidated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct View {
    pub review_id: String,
    pub date: DiaryDate,
    pub candidate_id: String,
    pub revision: Option<AuthenticRevision>,
    pub set_digest: String,
    /// The saved text: baseline plus accepted hunks.
    pub body: String,
    pub pending: Vec<Pending>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Receipt {
    pub hunk: String,
    pub revision: AuthenticRevision,
    pub before: String,
    pub after: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    pub receipt: Option<Receipt>,
    /// `None` when the review no longer exists (a replay after restart).
    pub view: Option<View>,
}

pub struct Decide {
    pub operation_id: OperationId,
    pub hunk: String,
    pub accept: bool,
    pub expected: Option<AuthenticRevision>,
    pub set_digest: String,
}

pub struct Start<'a> {
    pub session: &'a str,
    pub review_id: &'a str,
    pub date: DiaryDate,
    pub candidate_id: &'a str,
    pub candidate_body: &'a str,
    pub baseline: Option<AuthenticRevision>,
}

#[derive(Default)]
pub struct Reviews {
    inner: Mutex<BTreeMap<String, Review>>,
}

fn view_of(id: &str, r: &Review, h: &dyn Hasher) -> View {
    let (body, pending) = review::view(&r.base, &r.cand, &r.hunks, &r.accepted, &r.rejected);
    View {
        review_id: id.to_owned(),
        date: r.date,
        candidate_id: r.candidate_id.clone(),
        revision: r.revision.clone(),
        set_digest: review::set_digest(h, &r.accepted),
        body,
        pending,
    }
}

fn revision_of(loaded: Loaded) -> (Option<AuthenticRevision>, String) {
    match loaded {
        Loaded::Virtual(_) => (None, String::new()),
        Loaded::File { revision, body, .. } => (Some(revision), body),
    }
}

/// `seq:sha hunk,before,after` as stored in the operation result.
fn parse_receipt(result: &str) -> Option<Receipt> {
    let (head, tail) = result.split_once(' ')?;
    let (seq, sha) = head.split_once(':')?;
    let mut parts = tail.split(',');
    let receipt = Receipt {
        revision: AuthenticRevision {
            sequence: seq.parse().ok()?,
            sha256: sha.to_owned(),
        },
        hunk: parts.next()?.to_owned(),
        before: parts.next()?.to_owned(),
        after: parts.next()?.to_owned(),
    };
    parts.next().is_none().then_some(receipt)
}

impl Reviews {
    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, Review>> {
        self.inner.lock().expect("reviews poisoned")
    }

    /// Starts a review against the saved entry, which must still be the candidate's baseline.
    /// Writes nothing; an absent baseline is a virtual empty body.
    pub fn start(
        &self,
        store: &dyn AuthenticStore,
        h: &dyn Hasher,
        s: Start<'_>,
    ) -> Result<View, Error> {
        OperationId::parse(s.review_id).ok_or(Error::NotFound)?;
        let (revision, base) = revision_of(store.load(&s.date).map_err(Error::Store)?);
        if revision != s.baseline {
            return Err(Error::StaleBaseline);
        }
        let hunks = review::align(h, &base, s.candidate_body)?;
        let mut reviews = self.lock();
        if let Some(existing) = reviews.get(s.review_id) {
            return if existing.session == s.session {
                Ok(view_of(s.review_id, existing, h))
            } else {
                Err(Error::NotFound)
            };
        }
        // A reloaded client has lost its review ID; its idle review for the date is replaced.
        let old: Vec<(String, bool)> = reviews
            .iter()
            .filter(|(_, r)| r.session == s.session && r.date == s.date)
            .map(|(id, r)| (id.clone(), r.busy.is_some() || r.uncertain.is_some()))
            .collect();
        if old.iter().any(|(_, locked)| *locked) {
            return Err(Error::Busy);
        }
        for (id, _) in old {
            reviews.remove(&id);
        }
        let review = Review {
            session: s.session.to_owned(),
            date: s.date,
            candidate_id: s.candidate_id.to_owned(),
            candidate_digest: review::text_digest(h, s.candidate_body),
            base,
            cand: s.candidate_body.to_owned(),
            hunks,
            accepted: BTreeSet::new(),
            rejected: BTreeSet::new(),
            revision,
            busy: None,
            uncertain: None,
            invalidated: false,
        };
        let view = view_of(s.review_id, &review, h);
        reviews.insert(s.review_id.to_owned(), review);
        Ok(view)
    }

    pub fn decide(
        &self,
        store: &dyn AuthenticStore,
        h: &dyn Hasher,
        session: &str,
        review_id: &str,
        d: Decide,
    ) -> Result<Decision, Error> {
        if d.accept {
            // A known operation answers before any review-existence or staleness check.
            if let Some(record) = store.operation(&d.operation_id).map_err(Error::Store)? {
                if record.kind != KIND_REVIEW {
                    return Err(Error::Store(StoreError::Mismatch));
                }
                match record.state {
                    OperationState::Succeeded => {
                        let receipt = record
                            .result
                            .as_deref()
                            .and_then(parse_receipt)
                            .ok_or(Error::Store(StoreError::Storage))?;
                        return Ok(self.replay(h, session, review_id, receipt));
                    }
                    OperationState::Failed | OperationState::Cancelled => {
                        return Err(Error::Failed(record.result.unwrap_or_default()));
                    }
                    _ => {} // unfinished: reconciled by repeating the same save below
                }
            }
        }
        // Admit under the lock; the store call runs outside it with `busy` set.
        let save = {
            let mut reviews = self.lock();
            let r = reviews
                .get_mut(review_id)
                .filter(|r| r.session == session)
                .ok_or(Error::NotFound)?;
            if r.invalidated {
                return Err(Error::Invalidated);
            }
            if r.busy.is_some() {
                return Err(Error::Busy);
            }
            if r.uncertain.as_ref().is_some_and(|u| *u != d.operation_id) {
                return Err(Error::Uncertain);
            }
            if !r.hunks.iter().any(|x| x.id == d.hunk) {
                return Err(Error::NotFound);
            }
            let pending = !r.accepted.contains(&d.hunk) && !r.rejected.contains(&d.hunk);
            let retry = r.uncertain.as_ref() == Some(&d.operation_id);
            if !pending && !retry {
                return Err(Error::Decided);
            }
            if d.expected != r.revision {
                return Err(Error::StaleRevision);
            }
            let before = review::set_digest(h, &r.accepted);
            if d.set_digest != before {
                return Err(Error::SetMismatch);
            }
            if !d.accept {
                // Rejection writes nothing; confirm the entry is unchanged first.
                let (now, _) = revision_of(store.load(&r.date).map_err(Error::Store)?);
                if now != r.revision {
                    r.invalidated = true;
                    return Err(Error::Invalidated);
                }
                r.rejected.insert(d.hunk.clone());
                return Ok(Decision {
                    receipt: None,
                    view: Some(view_of(review_id, r, h)),
                });
            }
            let mut after_set = r.accepted.clone();
            after_set.insert(d.hunk.clone());
            let save = ReviewSave {
                operation_id: d.operation_id.clone(),
                date: r.date,
                expected: r.revision.clone(),
                body: review::compose(&r.base, &r.cand, &r.hunks, &after_set),
                candidate_digest: r.candidate_digest.clone(),
                hunk: d.hunk.clone(),
                before,
                after: review::set_digest(h, &after_set),
            };
            r.busy = Some(d.operation_id.clone());
            save
        };
        let result = store.save_review(save.clone());
        let mut reviews = self.lock();
        let Some(r) = reviews.get_mut(review_id) else {
            // Ended meanwhile (lease loss): the durable result stays recoverable by ID.
            return result.map_err(Error::Store).map(|out| Decision {
                receipt: Some(Receipt {
                    hunk: save.hunk,
                    revision: out.revision,
                    before: save.before,
                    after: save.after,
                }),
                view: None,
            });
        };
        r.busy = None;
        match result {
            Ok(out) => {
                r.accepted.insert(save.hunk.clone());
                r.revision = Some(out.revision.clone());
                r.uncertain = None;
                Ok(Decision {
                    receipt: Some(Receipt {
                        hunk: save.hunk,
                        revision: out.revision,
                        before: save.before,
                        after: save.after,
                    }),
                    view: Some(view_of(review_id, r, h)),
                })
            }
            Err(StoreError::Conflict(_)) => {
                r.invalidated = true;
                Err(Error::Invalidated)
            }
            Err(StoreError::Uncertain) => {
                r.uncertain = Some(d.operation_id);
                Err(Error::Uncertain)
            }
            Err(e) => Err(Error::Store(e)),
        }
    }

    /// Applies a committed receipt to a live review only when the set matches its before-digest;
    /// it never regresses later accepts and never saves again.
    fn replay(&self, h: &dyn Hasher, session: &str, review_id: &str, receipt: Receipt) -> Decision {
        let mut reviews = self.lock();
        let view = reviews
            .get_mut(review_id)
            .filter(|r| r.session == session)
            .map(|r| {
                if r.busy.is_none()
                    && !r.accepted.contains(&receipt.hunk)
                    && review::set_digest(h, &r.accepted) == receipt.before
                {
                    r.accepted.insert(receipt.hunk.clone());
                    r.revision = Some(receipt.revision.clone());
                    if r.uncertain.is_some() {
                        r.uncertain = None;
                    }
                }
                view_of(review_id, r, h)
            });
        Decision {
            receipt: Some(receipt),
            view,
        }
    }

    /// Ends the review. A decision in flight or an uncertain accept keeps it alive.
    pub fn end(&self, session: &str, review_id: &str) -> Result<(), Error> {
        let mut reviews = self.lock();
        match reviews.get(review_id).filter(|r| r.session == session) {
            None => Ok(()),
            Some(r) if r.busy.is_some() => Err(Error::Busy),
            Some(r) if r.uncertain.is_some() => Err(Error::Uncertain),
            Some(_) => {
                reviews.remove(review_id);
                Ok(())
            }
        }
    }

    /// True while a review uses the job's candidates (discard and regeneration are refused).
    pub fn uses_job(&self, session: &str, job_id: &str) -> bool {
        self.lock().values().any(|r| {
            r.session == session
                && r.candidate_id
                    .rsplit_once('-')
                    .is_some_and(|(job, _)| job == job_id)
        })
    }

    pub fn has_date(&self, session: &str, date: &DiaryDate) -> bool {
        self.lock()
            .values()
            .any(|r| r.session == session && r.date == *date)
    }

    /// Lease end: every transient review goes; accepted text is already saved.
    pub fn clear(&self) {
        self.lock().clear();
    }
}

#[cfg(test)]
mod tests;
