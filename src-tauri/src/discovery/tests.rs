use super::*;
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
};
fn fixture() -> (tempfile::TempDir, std::path::PathBuf, serde_json::Value) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let path = root.join("discovery.json");
    let value = serde_json::json!({"schema_version":1,"instance_id":"07249746-e280-4d51-a86a-cbcb198ee101","pid":1,"workspace_key":"b".repeat(64),"api_version":1,"origin":"http://127.0.0.1:8781","token":"a".repeat(64)});
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    (temp, path, value)
}
#[test]
fn malformed_discovery_never_becomes_a_connection() {
    let (_temp, path, valid) = fixture();
    assert!(read(&path).is_ok());
    for (key, bad) in [
        ("schema_version", serde_json::json!(2)),
        ("api_version", serde_json::json!(2)),
        ("pid", serde_json::json!(0)),
        ("instance_id", serde_json::json!("invalid")),
        ("workspace_key", serde_json::json!("invalid")),
        ("token", serde_json::json!("short")),
    ] {
        let mut value = valid.clone();
        value[key] = bad;
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(read(&path).is_err());
    }
    for origin in [
        "http://localhost:8781",
        "https://127.0.0.1:8781",
        "http://127.0.0.1:08781",
        "http://127.0.0.1:8781/",
        "http://user@127.0.0.1:8781",
        "http://127.0.0.1:8781?token=x",
        "http://[::1]:8781",
    ] {
        let mut value = valid.clone();
        value["origin"] = serde_json::json!(origin);
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(read(&path).is_err());
    }
    fs::write(&path, vec![b' '; 16385]).unwrap();
    assert!(read(&path).is_err());
    fs::write(&path, b"{\"schema_version\":1,\"schema_version\":1}").unwrap();
    assert!(read(&path).is_err());
}
#[test]
fn private_open_rejects_permissions_symlinks_and_preserves_opened_identity() {
    let (_temp, path, value) = fixture();
    let root = path.parent().unwrap();
    let opened = open(&path).unwrap();
    fs::rename(&path, root.join("old.json")).unwrap();
    fs::write(&path, b"hostile replacement").unwrap();
    assert_eq!(
        decode(opened).unwrap().token,
        value["token"].as_str().unwrap()
    );
    assert!(read(&path).is_err());
    fs::remove_file(&path).unwrap();
    symlink(root.join("old.json"), &path).unwrap();
    assert!(read(&path).is_err());
    fs::remove_file(&path).unwrap();
    fs::rename(root.join("old.json"), &path).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(read(&path).is_err());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    fs::set_permissions(root, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(read(&path).is_err());
    fs::set_permissions(root, fs::Permissions::from_mode(0o700)).unwrap();
    let alias = root.with_extension("alias");
    symlink(root, &alias).unwrap();
    assert!(read(&alias.join("discovery.json")).is_err());
    fs::remove_file(alias).unwrap();
}
