use super::*;
use std::os::unix::fs::PermissionsExt;

fn fixture() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().canonicalize().unwrap();
    let workspace = base.join("workspace");
    let runtime = base.join("runtime");
    for path in [&workspace, &runtime, &workspace.join(".diary")] {
        fs::create_dir(path).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let lock = workspace.join(".diary/workspace.lock");
    fs::write(&lock, "").unwrap();
    fs::set_permissions(lock, fs::Permissions::from_mode(0o600)).unwrap();
    (temp, workspace, runtime)
}

#[test]
fn os_lock_excludes_second_owner_and_releases_on_drop() {
    let (_temp, workspace, _) = fixture();
    let first = WorkspaceLock::acquire(&workspace).unwrap();
    assert!(matches!(
        WorkspaceLock::acquire(&workspace),
        Err(LifecycleError::WorkspaceBusy)
    ));
    drop(first);
    assert!(WorkspaceLock::acquire(&workspace).is_ok());
}

#[test]
fn uninitialized_workspace_is_not_modified() {
    let temp = tempfile::tempdir().unwrap();
    assert!(matches!(
        WorkspaceLock::acquire(&temp.path().canonicalize().unwrap()),
        Err(LifecycleError::WorkspaceUnavailable)
    ));
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
}

#[test]
fn discovery_roundtrip_is_private_rotates_credentials_and_cleans_own_record() {
    let (_temp, workspace, runtime) = fixture();
    let lock = WorkspaceLock::acquire(&workspace).unwrap();
    let first = Discovery::fresh(&lock.workspace_key, 8781).unwrap();
    let published = PublishedDiscovery::publish(&runtime, &first, &lock).unwrap();
    let read = read_discovery(&published.path).unwrap();
    assert_eq!(read.instance_id, first.instance_id);
    assert_eq!(read.token, first.token);
    let second = Discovery::fresh(&lock.workspace_key, 8782).unwrap();
    assert_ne!(first.token, second.token);
    assert_ne!(first.instance_id, second.instance_id);
    let replacement = PublishedDiscovery::publish(&runtime, &second, &lock).unwrap();
    let path = replacement.path.clone();
    drop(published);
    assert_eq!(
        read_discovery(&path).unwrap().instance_id,
        second.instance_id
    );
    drop(replacement);
    assert!(!path.exists());
}

#[test]
fn hostile_discovery_origins_and_schemas_are_rejected() {
    let (_temp, workspace, runtime) = fixture();
    let lock = WorkspaceLock::acquire(&workspace).unwrap();
    for origin in [
        "http://localhost:8781",
        "http://127.0.0.1:08781",
        "http://127.0.0.1:8781/",
        "http://127.0.0.1:8781?token=x",
        "http://user@127.0.0.1:8781",
        "https://127.0.0.1:8781",
        "http://[::1]:8781",
    ] {
        let mut record = Discovery::fresh(&lock.workspace_key, 8781).unwrap();
        record.origin = origin.into();
        assert_eq!(record.validate(), Err(LifecycleError::DiscoveryInvalid));
    }
    let record = Discovery::fresh(&lock.workspace_key, 8781).unwrap();
    let published = PublishedDiscovery::publish(&runtime, &record, &lock).unwrap();
    fs::write(&published.path, vec![b' '; 16 * 1024 + 1]).unwrap();
    assert!(matches!(
        read_discovery(&published.path),
        Err(LifecycleError::DiscoveryInvalid)
    ));
}

#[test]
fn occupied_prefix_selects_first_available_and_exhaustion_stops() {
    // Find a free contiguous test range without assuming production ports are free.
    let mut range = None;
    for base in (30_000..60_000).step_by(9) {
        let mut sockets = Vec::new();
        for port in base..base + 9 {
            if let Ok(socket) = TcpListener::bind((Ipv4Addr::LOCALHOST, port)) {
                sockets.push(socket);
            } else {
                break;
            }
        }
        if sockets.len() == 9 {
            range = Some((base, sockets));
            break;
        }
    }
    let (base, mut sockets) = range.expect("free fixture port range");
    assert!(matches!(
        bind_loopback(base, 8),
        Err(LifecycleError::PortsExhausted)
    ));
    assert!(matches!(
        bind_loopback(base, 0),
        Err(LifecycleError::PortsExhausted)
    ));
    drop(sockets.pop());
    let last = bind_loopback(base, 8).unwrap();
    assert_eq!(last.local_addr().unwrap().port(), base + 8);
    assert_eq!(last.local_addr().unwrap().ip(), Ipv4Addr::LOCALHOST);
    for (port, count) in [(1023, 0), (8781, 9), (65535, 1)] {
        assert!(matches!(
            bind_loopback(port, count),
            Err(LifecycleError::InvalidPorts)
        ));
    }
}

#[test]
fn cleanup_stays_in_opened_directory_after_runtime_name_replacement() {
    let (_temp, workspace, runtime) = fixture();
    let lock = WorkspaceLock::acquire(&workspace).unwrap();
    let record = Discovery::fresh(&lock.workspace_key, 8781).unwrap();
    let published = PublishedDiscovery::publish(&runtime, &record, &lock).unwrap();
    let moved = runtime.with_file_name("moved-runtime");
    fs::rename(&runtime, &moved).unwrap();
    fs::create_dir(&runtime).unwrap();
    fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();
    let replacement = runtime.join(&lock.workspace_key);
    fs::create_dir(&replacement).unwrap();
    fs::set_permissions(&replacement, fs::Permissions::from_mode(0o700)).unwrap();
    let sentinel = replacement.join("discovery.json");
    fs::write(&sentinel, b"replacement sentinel").unwrap();
    drop(published);
    assert_eq!(fs::read(&sentinel).unwrap(), b"replacement sentinel");
    assert!(
        !moved
            .join(&lock.workspace_key)
            .join("discovery.json")
            .exists()
    );
}

#[test]
fn publication_rejects_symlinked_runtime_or_workspace_key_directory() {
    use std::os::unix::fs::symlink;
    let (_temp, workspace, runtime) = fixture();
    let lock = WorkspaceLock::acquire(&workspace).unwrap();
    let record = Discovery::fresh(&lock.workspace_key, 8781).unwrap();
    let alias = runtime.with_file_name("runtime-alias");
    symlink(&runtime, &alias).unwrap();
    assert!(matches!(
        PublishedDiscovery::publish(&alias, &record, &lock),
        Err(LifecycleError::UnsafePath)
    ));
    let outside = runtime.with_file_name("outside");
    fs::create_dir(&outside).unwrap();
    fs::set_permissions(&outside, fs::Permissions::from_mode(0o700)).unwrap();
    symlink(&outside, runtime.join(&lock.workspace_key)).unwrap();
    assert!(matches!(
        PublishedDiscovery::publish(&runtime, &record, &lock),
        Err(LifecycleError::UnsafePath)
    ));
    assert_eq!(fs::read_dir(&outside).unwrap().count(), 0);
}

#[test]
fn cleanup_preserves_foreign_malformed_and_symlinked_records() {
    use std::os::unix::fs::symlink;
    for replacement in ["malformed", "symlink"] {
        let (_temp, workspace, runtime) = fixture();
        let lock = WorkspaceLock::acquire(&workspace).unwrap();
        let record = Discovery::fresh(&lock.workspace_key, 8781).unwrap();
        let published = PublishedDiscovery::publish(&runtime, &record, &lock).unwrap();
        let path = published.path.clone();
        fs::remove_file(&path).unwrap();
        let sentinel = runtime.join("sentinel");
        fs::write(&sentinel, b"untouched").unwrap();
        if replacement == "malformed" {
            fs::write(&path, b"foreign malformed record").unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        } else {
            symlink(&sentinel, &path).unwrap();
        }
        drop(published);
        assert_eq!(fs::read(&sentinel).unwrap(), b"untouched");
        if replacement == "malformed" {
            assert_eq!(fs::read(path).unwrap(), b"foreign malformed record");
        } else {
            assert!(fs::symlink_metadata(path).unwrap().is_symlink());
        }
    }
}

#[test]
fn workspace_key_matches_lock_key() {
    let (_temp, workspace, _) = fixture();
    let lock = WorkspaceLock::acquire(&workspace).unwrap();
    assert_eq!(workspace_key(&workspace), lock.workspace_key);
}

#[test]
fn entropy_fills_distinct_buffers() {
    let (mut a, mut b) = ([0u8; 16], [0u8; 16]);
    entropy(&mut a).unwrap();
    entropy(&mut b).unwrap();
    assert_ne!(a, b);
    assert_ne!(a, [0u8; 16]);
}
