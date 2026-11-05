use super::*;

#[test]
fn opened_metadata_policy_rejects_foreign_owners_kinds_and_special_modes() {
    assert_eq!(
        private_attributes(501, 501, u32::from(libc::S_IFREG | 0o600), false),
        Ok(())
    );
    assert_eq!(
        private_attributes(501, 501, u32::from(libc::S_IFDIR | 0o700), true),
        Ok(())
    );
    for (owner, mode, directory) in [
        (502, libc::S_IFREG | 0o600, false),
        (0, libc::S_IFDIR | 0o700, true),
        (501, libc::S_IFLNK | 0o600, false),
        (501, libc::S_IFDIR | 0o600, false),
        (501, libc::S_IFREG | 0o700, true),
        (501, libc::S_IFREG | 0o4600, false),
        (501, libc::S_IFREG | 0o660, false),
        (501, libc::S_IFDIR | 0o750, true),
    ] {
        assert_eq!(
            private_attributes(owner, 501, u32::from(mode), directory),
            Err(FsError::Permissions)
        );
    }
}

fn dir() -> (tempfile::TempDir, std::path::PathBuf, PrivateDir) {
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().canonicalize().unwrap();
    std::fs::set_permissions(&base, std::os::unix::fs::PermissionsExt::from_mode(0o700)).unwrap();
    let dir = PrivateDir::open(&base).unwrap();
    (temp, base, dir)
}

#[test]
fn list_returns_names_and_hostile_entries_fail_metadata() {
    let (_t, base, dir) = dir();
    std::fs::write(base.join("reg"), "abc").unwrap();
    std::os::unix::fs::symlink(base.join("reg"), base.join("link")).unwrap();
    let fifo = std::ffi::CString::new(base.join("fifo").to_str().unwrap()).unwrap();
    // SAFETY: valid NUL-terminated path.
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
    assert_eq!(dir.list().unwrap(), ["fifo", "link", "reg"]);
    let meta = dir.metadata("reg").unwrap();
    assert_eq!(meta.len, 3);
    assert!(meta.modified_epoch_secs > 1_700_000_000);
    assert!(dir.metadata("link").is_err());
    assert!(dir.metadata("fifo").is_err());
    assert!(dir.metadata("missing").is_err());
}

#[test]
fn lock_is_exclusive_across_handles() {
    let (_t, _base, dir) = dir();
    let first = dir.lock("w.lock").unwrap();
    assert_eq!(dir.lock("w.lock").err(), Some(FsError::Busy));
    drop(first);
    assert!(dir.lock("w.lock").is_ok());
}

#[test]
fn append_creates_0600_file_appends_and_rejects_symlink() {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    let (_t, base, dir) = dir();
    dir.append("a.log").unwrap().write_all(b"one\n").unwrap();
    dir.append("a.log").unwrap().write_all(b"two\n").unwrap();
    assert_eq!(std::fs::read(base.join("a.log")).unwrap(), b"one\ntwo\n");
    assert_eq!(
        std::fs::metadata(base.join("a.log"))
            .unwrap()
            .permissions()
            .mode()
            & 0o7777,
        0o600
    );
    std::os::unix::fs::symlink(base.join("a.log"), base.join("b.log")).unwrap();
    assert!(dir.append("b.log").is_err());
}

#[test]
fn open_child_does_not_create_and_read_bounded_caps_bytes() {
    let (_t, base, dir) = dir();
    assert!(dir.open_child("missing").is_err());
    assert!(!base.join("missing").exists());
    std::fs::create_dir(base.join("sub")).unwrap();
    std::fs::set_permissions(
        base.join("sub"),
        std::os::unix::fs::PermissionsExt::from_mode(0o700),
    )
    .unwrap();
    assert!(dir.open_child("sub").is_ok());
    std::fs::write(base.join("big"), vec![b'x'; 100]).unwrap();
    assert_eq!(dir.read_bounded("big", 10).unwrap().len(), 10);
    assert_eq!(dir.read_bounded("big", 1000).unwrap().len(), 100);
    std::os::unix::fs::symlink(base.join("big"), base.join("link")).unwrap();
    assert!(dir.read_bounded("link", 10).is_err());
}
