//! `ProfileService` with a Qwen profile: real status through the adapter, capabilities from the
//! embedded assets, no run started.
use diary_application::{agent::*, diagnostics::NullDiagnostics};
mod support;
use diary_infrastructure::agent::{
    process::Limits,
    profiles::{Entry, ProfileService, ProfileTable},
    registry::Agents,
};
use std::sync::Arc;

fn service(home: Option<&std::path::Path>) -> ProfileService {
    let exe = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/fake_cli");
    let root = tempfile::tempdir().unwrap().keep();
    let table = Arc::new(ProfileTable::from_entries(vec![Entry {
        alias: "q".into(),
        kind: AgentKind::Qwen,
        home: home.map(std::path::Path::to_path_buf),
        executable: Some(exe),
    }]));
    let host = support::host(&root, Arc::new(NullDiagnostics), Limits::production());
    let agents = Arc::new(Agents::standard(table.clone(), host));
    ProfileService::new(table, agents)
}

#[test]
fn qwen_profile_reports_real_status_and_capability() {
    let dir = tempfile::tempdir().unwrap();
    let s = service(Some(dir.path()));
    let listed = s.list();
    assert_eq!(listed[0].status, None, "listing spawns nothing");
    assert_eq!(listed[0].chat, Capability::Ready);
    assert_eq!(listed[0].generate, Capability::Ready);
    assert_eq!(s.check("q").unwrap().status, Some(ProfileStatus::Ready));
    assert_eq!(s.list()[0].status, Some(ProfileStatus::Ready));
    assert!(s.check("nope").is_none());

    std::fs::write(dir.path().join("version"), "0.27.0").unwrap();
    let s = service(Some(dir.path()));
    assert_eq!(
        s.check("q").unwrap().status,
        Some(ProfileStatus::UnsupportedVersion)
    );
    assert_eq!(
        service(None).check("q").unwrap().status,
        Some(ProfileStatus::Unavailable)
    );
}
