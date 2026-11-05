use super::*;
use std::os::unix::fs::{PermissionsExt, symlink};

#[test]
fn rename_new_never_clobbers() {
    let d = tempfile::tempdir().unwrap();
    let (a, b) = (d.path().join("a"), d.path().join("b"));
    create_new(&a, b"new").unwrap();
    create_new(&b, b"old").unwrap();
    assert_eq!(
        rename_new(&a, &b).unwrap_err().kind(),
        io::ErrorKind::AlreadyExists
    );
    assert_eq!(std::fs::read(&b).unwrap(), b"old");
    remove(&b).unwrap();
    rename_new(&a, &b).unwrap();
    assert_eq!(std::fs::read(&b).unwrap(), b"new");
}

#[test]
fn replace_is_atomic_and_syncs() {
    let d = tempfile::tempdir().unwrap();
    let (a, b) = (d.path().join("a"), d.path().join("b"));
    create_new(&a, b"new").unwrap();
    create_new(&b, b"old").unwrap();
    sync_file(&a).unwrap();
    rename_replace(&a, &b).unwrap();
    sync_file(&b).unwrap();
    sync_path(d.path()).unwrap();
    assert_eq!(std::fs::read(&b).unwrap(), b"new");
}

#[test]
fn create_new_is_exclusive_and_private() {
    let d = tempfile::tempdir().unwrap();
    let a = d.path().join("a");
    create_new(&a, b"x").unwrap();
    assert!(create_new(&a, b"y").is_err());
    assert_eq!(
        std::fs::metadata(&a).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[test]
fn ensure_dirs_builds_private_chain_and_refuses_symlinks() {
    let d = tempfile::tempdir().unwrap();
    ensure_dirs(d.path(), &["assets", "diary", "2026", "11"]).unwrap();
    let leaf = d.path().join("assets/diary/2026/11");
    assert_eq!(
        std::fs::metadata(&leaf).unwrap().permissions().mode() & 0o777,
        0o700
    );
    ensure_dirs(d.path(), &["assets", "diary", "2026", "11"]).unwrap();
    let target = tempfile::tempdir().unwrap();
    symlink(target.path(), d.path().join("link")).unwrap();
    assert!(ensure_dirs(d.path(), &["link", "x"]).is_err());
    assert!(!target.path().join("x").exists());
}

#[test]
fn read_bounded_refuses_symlinks_and_large_files() {
    let d = tempfile::tempdir().unwrap();
    let a = d.path().join("a");
    assert_eq!(read_bounded(&a, 10).unwrap(), None);
    create_new(&a, b"hello").unwrap();
    assert_eq!(read_bounded(&a, 10).unwrap().unwrap().bytes, b"hello");
    assert!(read_bounded(&a, 4).is_err());
    symlink(&a, d.path().join("l")).unwrap();
    assert!(read_bounded(&d.path().join("l"), 10).is_err());
}

#[test]
fn durability_primitives_run_on_apfs() {
    // Evidence for the supported filesystem: the temp volume is APFS, where F_FULLFSYNC and
    // renamex_np(RENAME_EXCL) are available (both are exercised by the tests above).
    let d = tempfile::tempdir().unwrap();
    let path = std::ffi::CString::new(d.path().as_os_str().as_bytes()).unwrap();
    let mut stat: libc::statfs = unsafe { std::mem::zeroed() };
    // SAFETY: valid NUL-terminated path and writable statfs buffer.
    assert_eq!(unsafe { libc::statfs(path.as_ptr(), &mut stat) }, 0);
    let name: Vec<u8> = stat
        .f_fstypename
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as u8)
        .collect();
    assert_eq!(String::from_utf8_lossy(&name), "apfs");
}
