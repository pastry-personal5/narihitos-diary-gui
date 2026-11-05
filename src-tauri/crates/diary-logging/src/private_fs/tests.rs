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
