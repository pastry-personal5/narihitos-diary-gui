//! Framework-independent protocol compatibility values and the Authentic file format.

pub mod authentic;
pub mod calendar;
pub mod review;
pub mod source;

/// Compatibility is explicit: every required capability must be advertised.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolVersion {
    pub major: u16,
    pub minor: u16,
}

impl ProtocolVersion {
    pub fn accepts(self, server: Self, required: &[&str], available: &[&str]) -> bool {
        self.major == server.major
            && server.minor >= self.minor
            && required.iter().all(|name| available.contains(name))
    }
}

#[cfg(test)]
mod tests {
    use super::ProtocolVersion;

    #[test]
    fn incompatible_peers_cannot_pass_handshake() {
        let client = ProtocolVersion { major: 1, minor: 1 };
        assert!(client.accepts(client, &["status"], &["status"]));
        assert!(!client.accepts(ProtocolVersion { major: 2, minor: 1 }, &[], &[]));
        assert!(!client.accepts(ProtocolVersion { major: 1, minor: 0 }, &[], &[]));
        assert!(!client.accepts(client, &["storage"], &["status"]));
    }
}
