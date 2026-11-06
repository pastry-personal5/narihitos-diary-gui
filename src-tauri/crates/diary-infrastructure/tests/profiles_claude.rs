//! `ProfileService` with a Claude profile: real status through the adapter, capabilities from the
//! embedded assets, no run started.
use diary_application::{agent::*, diagnostics::NullDiagnostics};
mod support;
use diary_infrastructure::agent::{
    process::Limits,
    profiles::{Entry, ProfileService, ProfileTable},
    registry::Agents,
};
use std::sync::Arc;

fn service(home: &std::path::Path, login: &str) -> ProfileService {
    std::fs::write(home.join("login"), login).unwrap();
    let exe = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/fake_cli");
    let table = Arc::new(ProfileTable::from_entries(vec![Entry {
        alias: "c".into(),
        kind: AgentKind::Claude,
        home: Some(home.to_path_buf()),
        executable: Some(exe),
    }]));
    let host = support::host(home, Arc::new(NullDiagnostics), Limits::production());
    let agents = Arc::new(Agents::standard(table.clone(), host));
    ProfileService::new(table, agents)
}

#[test]
fn claude_profile_reports_real_status_and_capability() {
    let dir = tempfile::tempdir().unwrap();
    let s = service(dir.path(), "");
    let listed = s.list();
    assert_eq!(listed[0].status, None, "listing spawns nothing");
    assert_eq!(listed[0].chat, Capability::Ready);
    assert_eq!(listed[0].generate, Capability::Ready);
    assert_eq!(s.check("c").unwrap().status, Some(ProfileStatus::Ready));
    assert_eq!(s.list()[0].status, Some(ProfileStatus::Ready));
    assert!(s.check("nope").is_none());

    let dir = tempfile::tempdir().unwrap();
    let s = service(dir.path(), "none");
    assert_eq!(
        s.check("c").unwrap().status,
        Some(ProfileStatus::Unauthenticated)
    );
}
