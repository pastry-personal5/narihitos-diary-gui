//! Request-data encoding: the `{{data}}` JSON for a prompt, bounded after encoding.
use diary_application::agent::{
    AgentError, AgentErrorCode, DocumentKind, GenerationPacket, MAX_PACKET_BYTES, Mode, Role,
    SCHEMA_VERSION,
};
use serde_json::{Value, json};

pub fn encode_packet(mode: &Mode) -> Result<String, AgentError> {
    let v = match mode {
        Mode::Chat(c) => json!({
            "schema_version": SCHEMA_VERSION,
            "document": c.document().map(|d| json!({
                "kind": match d.kind { DocumentKind::Authentic => "authentic", DocumentKind::Source => "source" },
                "date": d.date.to_string(),
                "unsaved": d.unsaved,
                "body": d.body,
            })),
            "transcript": c.transcript().iter().map(|t| json!({
                "role": match t.role { Role::User => "user", Role::Assistant => "assistant" },
                "text": t.text,
            })).collect::<Vec<Value>>(),
            "user_message": c.user_message(),
        }),
        Mode::Candidate { packet, .. } => generation(packet),
    };
    let s = v.to_string();
    if s.len() > MAX_PACKET_BYTES {
        return Err(AgentErrorCode::Context.into());
    }
    Ok(s)
}

fn generation(p: &GenerationPacket) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "style": p.style().id(),
        "target_date": p.target_date().to_string(),
        "snippets": p.snippets().iter().map(|s| json!({
            "id": s.id, "instant_utc": s.instant_utc, "civil_date": s.civil_date,
            "offset": s.offset, "body": s.body,
        })).collect::<Vec<Value>>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use diary_application::agent::{ChatPacket, SnippetItem, Style};
    use diary_domain::authentic::DiaryDate;

    fn candidate(body: String) -> Mode {
        let s = SnippetItem {
            id: "snippet-1".into(),
            instant_utc: "2026-11-06T01:00:00Z".into(),
            civil_date: "2026-11-06".into(),
            offset: "+09:00".into(),
            body,
        };
        let packet = GenerationPacket::new(
            Style::Concise,
            DiaryDate::from_ymd(2026, 11, 6).unwrap(),
            vec![s],
        )
        .unwrap();
        Mode::Candidate {
            index: 1,
            count: 1,
            packet,
        }
    }

    #[test]
    fn encoded_packet_over_256kib_is_context() {
        let ok = encode_packet(&candidate("a".into())).unwrap();
        assert!(ok.contains("\"style\":\"concise\"") && ok.contains("snippet-1"));
        assert!(!ok.contains("language") && !ok.contains("reviewed_memory"));
        // 250 KiB of body is under the bound raw, 256 KiB of quotes is over it once escaped.
        assert!(encode_packet(&candidate("a".repeat(250 * 1024))).is_ok());
        let e = encode_packet(&candidate("\"".repeat(200 * 1024))).unwrap_err();
        assert_eq!(e.0, AgentErrorCode::Context);
    }

    #[test]
    fn chat_without_document_has_null_document() {
        let m = Mode::Chat(ChatPacket::new(None, vec![], "hi".into()).unwrap());
        assert!(encode_packet(&m).unwrap().contains("\"document\":null"));
    }
}
