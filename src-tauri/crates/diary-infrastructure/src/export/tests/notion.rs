use super::*;
use crate::export::notion::{
    client::Client,
    fake::{FakeNotion, Fault},
};
const ROOT: &str = "0123456789abcdef0123456789abcdef";

struct N {
    e: Env,
    fake: Arc<FakeNotion>,
    svc: ExportCoordinator,
}
fn n() -> N {
    let e = env();
    let fake = Arc::new(FakeNotion::new(ROOT));
    let client = Client::new(fake.clone(), Arc::new(|_| {}), Duration::ZERO);
    let notion = e.ws.notion(client, ROOT).unwrap();
    let svc =
        e.ws.exports_with(e.store.clone(), vec![e.fake.clone(), Arc::new(notion)])
            .unwrap();
    svc.recover().unwrap();
    N { e, fake, svc }
}
impl N {
    fn req(&self, id: u8, date: &str, rev: &AuthenticRevision) -> StartRequest {
        StartRequest {
            target: Target::Notion,
            ..self.e.request(id, date, rev)
        }
    }
    fn state(&self, id: u8) -> ExportResult {
        self.svc.operation(&op(id)).unwrap().unwrap()
    }
    fn wait(&self, id: u8, want: ExportState) {
        for _ in 0..400 {
            if self.state(id).state == want {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!(
            "operation {id} is {:?}, wanted {want:?}",
            self.state(id).state
        );
    }
    /// The blocks of the first day: everything after its heading up to the next day heading.
    fn body(&self) -> Vec<String> {
        let year = self.fake.children(ROOT)[0].clone();
        let month = self.fake.children(&year)[0].clone();
        self.fake
            .children(&month)
            .into_iter()
            .skip(1)
            .take_while(|c| self.fake.kind_of(c) != "heading_2")
            .collect()
    }
}

#[test]
fn both_targets_are_available_and_notion_creates_then_verifies_a_noop() {
    let n = n();
    assert!(n.svc.available(Target::Icloud) && n.svc.available(Target::Notion));
    let rev = n.e.save(1, "2026-11-05", None, "one\n\ntwo\n");
    n.svc.start("s", 1, n.req(1, "2026-11-05", &rev)).unwrap();
    n.wait(1, ExportState::Succeeded);
    assert_eq!(n.body().len(), 2);
    assert_eq!(n.state(1).reason, None);
    let appends = n.fake.count("append");
    // Same revision again: a verified no-op writes nothing.
    n.svc.start("s", 1, n.req(2, "2026-11-05", &rev)).unwrap();
    n.wait(2, ExportState::Succeeded);
    assert_eq!(n.fake.count("append"), appends);
    // The iCloud destination is independent and still empty.
    assert!(n.e.remote_text("2026-11-05").is_none());
}

#[test]
fn an_external_edit_needs_confirmation_before_replacement() {
    let n = n();
    let rev = n.e.save(1, "2026-11-05", None, "one\n");
    n.svc.start("s", 1, n.req(1, "2026-11-05", &rev)).unwrap();
    n.wait(1, ExportState::Succeeded);
    let child = n.body()[0].clone();
    n.fake.edit_text(&child, "edited in Notion");
    let rev2 = n.e.save(2, "2026-11-05", Some(&rev), "two\n");
    let Started::Preview(p) = n.svc.start("s", 1, n.req(2, "2026-11-05", &rev2)).unwrap() else {
        panic!("wanted a preview");
    };
    assert_eq!((p.reason, p.write), (Reason::Changed, Write::Replace));
    n.svc
        .confirm(
            "s",
            1,
            ConfirmRequest {
                operation_id: p.operation_id.clone(),
                step_id: step(9),
                expected_checkpoint: p.expected_checkpoint,
                token: p.token.clone(),
            },
        )
        .unwrap();
    n.wait(2, ExportState::Succeeded);
    let kids = n.body();
    assert_eq!(kids.len(), 1);
    assert_eq!(n.fake.text_of(&kids[0]), "two");
}

#[test]
fn unsupported_markdown_succeeds_with_a_warning_tag() {
    let n = n();
    let rev = n.e.save(1, "2026-11-05", None, "| a |\n|---|\n| 1 |\n");
    n.svc.start("s", 1, n.req(1, "2026-11-05", &rev)).unwrap();
    n.wait(1, ExportState::Succeeded);
    assert_eq!(n.state(1).reason.as_deref(), Some("projection_warnings"));
}

#[test]
fn a_lost_reply_after_the_last_chunk_is_proven_by_read_back() {
    let n = n();
    // Call 1 creates the section, call 2 appends the only chunk.
    n.fake.fault_after("append", 1, Fault::LoseReply);
    let rev = n.e.save(1, "2026-11-05", None, "one\n");
    n.svc.start("s", 1, n.req(1, "2026-11-05", &rev)).unwrap();
    n.wait(1, ExportState::Succeeded);
    assert_eq!(n.fake.count("append"), 2);
}

#[test]
fn a_lost_reply_mid_write_parks_uncertain_and_is_never_replayed() {
    let n = n();
    n.fake.fault_after("append", 0, Fault::LoseReply);
    let rev = n.e.save(1, "2026-11-05", None, "one\n");
    n.svc.start("s", 1, n.req(1, "2026-11-05", &rev)).unwrap();
    n.wait(1, ExportState::Uncertain);
    assert_eq!(n.fake.count("append"), 1);
    // A second export to the same root is blocked by the unresolved one.
    let again = n.svc.start("s", 1, n.req(2, "2026-11-05", &rev));
    assert_eq!(again.unwrap_err(), ExportError::Uncertain);
    // Restart recovery reads back and keeps it unresolved, without writing.
    let svc = n.e.ws.exports_with(n.e.store.clone(), vec![]).unwrap();
    svc.recover().unwrap();
    assert_eq!(n.fake.count("append"), 1);
}

#[test]
fn typed_unavailability_reaches_the_caller() {
    let n = n();
    let rev = n.e.save(1, "2026-11-05", None, "one\n");
    n.fake.fault("list", Fault::Status(401));
    assert_eq!(
        n.svc
            .start("s", 1, n.req(1, "2026-11-05", &rev))
            .unwrap_err(),
        ExportError::CloudUnavailable(Unavailable::Unauthorized)
    );
}
