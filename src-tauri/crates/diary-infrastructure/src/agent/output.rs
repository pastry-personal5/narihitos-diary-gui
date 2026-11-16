//! Strict generation schema-1 validation. Derived structs reject unknown and duplicate keys; there
//! is no extraction fallback for fenced or prose-wrapped payloads.
use diary_application::agent::{
    AgentError, AgentErrorCode, AgentOutput, Candidate, Claim, MAX_RESULT_BYTES, MAX_TEXT_BYTES,
};
use serde::Deserialize;

const MAX_SUPPORT: usize = 2_000;
const MAX_NOTE_BYTES: usize = 2 * 1024;
const MAX_WARNINGS: usize = 100;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    draft_markdown: String,
    claim_support: Vec<RawClaim>,
    warnings: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawClaim {
    claim: String,
    snippet_ids: Vec<String>,
}

fn bad() -> AgentError {
    AgentErrorCode::InvalidOutput.into()
}

fn valid_id(id: &str, count: usize) -> bool {
    id.strip_prefix("snippet-").is_some_and(|n| {
        !n.starts_with('0')
            && !n.is_empty()
            && n.bytes().all(|b| b.is_ascii_digit())
            && n.parse::<usize>().is_ok_and(|n| (1..=count).contains(&n))
    })
}

/// A chat reply is plain text bounded like a draft; nothing is truncated.
pub fn validate_reply(text: String) -> Result<AgentOutput, AgentError> {
    if text.len() > MAX_TEXT_BYTES {
        return Err(bad());
    }
    Ok(AgentOutput::Reply(text))
}

pub fn validate_generation(bytes: &[u8], snippet_count: usize) -> Result<Candidate, AgentError> {
    if bytes.len() > MAX_RESULT_BYTES {
        return Err(bad());
    }
    let raw: Raw = serde_json::from_slice(bytes).map_err(|_| bad())?;
    let d = &raw.draft_markdown;
    if d.len() > MAX_TEXT_BYTES
        || d.trim().is_empty()
        || d.trim_start_matches(|c: char| c.is_whitespace() || c == '\u{feff}')
            .lines()
            .next()
            .is_some_and(|l| l.trim_end() == "---")
        || raw.claim_support.len() > MAX_SUPPORT
        || raw.warnings.len() > MAX_WARNINGS
        || raw.warnings.iter().any(|w| w.len() > MAX_NOTE_BYTES)
    {
        return Err(bad());
    }
    for c in &raw.claim_support {
        if c.claim.len() > MAX_NOTE_BYTES
            || c.snippet_ids.is_empty()
            || c.snippet_ids.len() > snippet_count
            || !c.snippet_ids.iter().all(|i| valid_id(i, snippet_count))
            || c.snippet_ids
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
                != c.snippet_ids.len()
        {
            return Err(bad());
        }
    }
    Ok(Candidate {
        draft_markdown: raw.draft_markdown,
        claim_support: raw
            .claim_support
            .into_iter()
            .map(|c| Claim {
                claim: c.claim,
                snippet_ids: c.snippet_ids,
            })
            .collect(),
        warnings: raw.warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn json(draft: &str, claims: &str, warnings: &str) -> String {
        format!(r#"{{"draft_markdown":{draft},"claim_support":{claims},"warnings":{warnings}}}"#)
    }
    fn fails(s: &str) -> bool {
        validate_generation(s.as_bytes(), 2).unwrap_err().0 == AgentErrorCode::InvalidOutput
    }

    #[test]
    fn size_bounds() {
        let big = format!("\"{}\"", "a".repeat(MAX_TEXT_BYTES + 1));
        assert!(fails(&json(&big, "[]", "[]")));
        let edge = format!("\"{}\"", "a".repeat(MAX_TEXT_BYTES - 1));
        assert!(validate_generation(json(&edge, "[]", "[]").as_bytes(), 2).is_ok());
        let many = format!(
            "[{}]",
            vec![r#"{"claim":"c","snippet_ids":["snippet-1"]}"#; MAX_SUPPORT + 1].join(",")
        );
        assert!(fails(&json("\"x\"", &many, "[]")));
        let long = format!(
            r#"[{{"claim":"{}","snippet_ids":["snippet-1"]}}]"#,
            "c".repeat(MAX_NOTE_BYTES + 1)
        );
        assert!(fails(&json("\"x\"", &long, "[]")));
        let warns = format!("[{}]", vec!["\"w\""; MAX_WARNINGS + 1].join(","));
        assert!(fails(&json("\"x\"", "[]", &warns)));
        let warn = format!("[\"{}\"]", "w".repeat(MAX_NOTE_BYTES + 1));
        assert!(fails(&json("\"x\"", "[]", &warn)));
        let huge = json(&format!("\"{}\"", "a".repeat(MAX_RESULT_BYTES)), "[]", "[]");
        assert!(fails(&huge));
    }

    #[test]
    fn front_matter_after_bom_is_rejected() {
        assert!(fails(&json("\"\u{feff}---\\na: b\\n---\\nx\"", "[]", "[]")));
    }

    #[test]
    fn reply_bound() {
        assert!(validate_reply("a".repeat(MAX_TEXT_BYTES)).is_ok());
        let e = validate_reply("a".repeat(MAX_TEXT_BYTES + 1)).unwrap_err();
        assert_eq!(e.0, AgentErrorCode::InvalidOutput);
    }

    #[test]
    fn id_bounds() {
        let c = |ids: &str| {
            json(
                "\"x\"",
                &format!(r#"[{{"claim":"c","snippet_ids":{ids}}}]"#),
                "[]",
            )
        };
        assert!(validate_generation(c(r#"["snippet-2"]"#).as_bytes(), 2).is_ok());
        for ids in [
            r#"[]"#,
            r#"["snippet-3"]"#,
            r#"["snippet-0"]"#,
            r#"["snippet-01"]"#,
            r#"["snippet-+1"]"#,
            r#"["x"]"#,
            r#"["snippet-1","snippet-2","snippet-1"]"#,
        ] {
            assert!(fails(&c(ids)), "{ids}");
        }
    }
}

/// Strict illustrated response; no prose/fence extraction or partial acceptance.
#[cfg(test)]
pub fn validate_illustrated(
    bytes: &[u8],
    cap: usize,
) -> Result<diary_application::illustrated::Plan, AgentError> {
    validate_illustrated_detailed(bytes, cap).map_err(|_| bad())
}

pub fn validate_illustrated_detailed(
    bytes: &[u8],
    cap: usize,
) -> Result<diary_application::illustrated::Plan, diary_application::illustrated::ValidationFailure>
{
    use diary_application::{diagnostics::ValidationRule, illustrated::ValidationFailure};
    let fail = |rule| ValidationFailure {
        rule,
        group_index: None,
    };
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Raw {
        themes: Vec<String>,
        groups: Vec<Group>,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Group {
        themes: Vec<usize>,
        text: String,
        alt: String,
        prompt: String,
    }
    if bytes.len() > MAX_RESULT_BYTES {
        return Err(fail(ValidationRule::ResultTooLarge));
    }
    let raw: Raw = serde_json::from_slice(bytes).map_err(|e| {
        fail(match e.classify() {
            serde_json::error::Category::Data => ValidationRule::JsonSchema,
            _ => ValidationRule::JsonSyntax,
        })
    })?;
    let plan = diary_application::illustrated::Plan {
        themes: raw.themes,
        groups: raw
            .groups
            .into_iter()
            .map(|g| diary_application::illustrated::Group {
                themes: g.themes,
                text: g.text,
                alt: g.alt,
                prompt: g.prompt,
            })
            .collect(),
    };
    plan.validate_detailed(cap)?;
    Ok(plan)
}

#[cfg(test)]
mod illustrated_tests {
    use super::*;
    #[test]
    fn themes_must_be_covered_once_and_cap_enforced() {
        let good=br#"{"themes":["walk","work"],"groups":[{"themes":[1,2],"text":"Walked, then worked.","alt":"Morning","prompt":"morning light"}]}"#;
        assert!(validate_illustrated(good, 1).is_ok());
        for bad in [
            String::from_utf8_lossy(good).replace("[1,2]", "[1,1]"),
            String::from_utf8_lossy(good).replace("[1,2]", "[1]"),
            String::from_utf8_lossy(good).replace("morning light", ""),
            format!("```{}```", String::from_utf8_lossy(good)),
        ] {
            assert!(validate_illustrated(bad.as_bytes(), 4).is_err());
        }
        assert!(validate_illustrated(good, 0).is_err());
    }
}

#[cfg(test)]
mod illustrated_diagnostic_tests {
    use super::*;
    use diary_application::diagnostics::ValidationRule::*;
    use serde_json::json;

    #[test]
    fn rejection_rules_identify_the_failed_constraint_without_content() {
        let good = json!({"themes":["private theme"],"groups":[{"themes":[1],"text":"private diary", "alt":"private alt", "prompt":"private prompt"}]});
        let check = |v: serde_json::Value, cap, rule, group_index| {
            let failure =
                validate_illustrated_detailed(&serde_json::to_vec(&v).unwrap(), cap).unwrap_err();
            assert_eq!(failure.rule, rule);
            assert_eq!(failure.group_index, group_index);
            assert!(!format!("{failure:?}").contains("private"));
        };
        for (field, value, rule) in [
            ("themes", json!([]), GroupThemesEmpty),
            ("text", json!(" "), TextEmpty),
            ("text", json!("private\rdiary"), TextControl),
            ("text", json!("![private](image)"), TextImage),
            ("text", json!("<private>"), TextHtml),
            ("alt", json!(""), AltEmpty),
            ("alt", json!("x".repeat(1025)), AltTooLarge),
            ("alt", json!("[private]"), AltCharacters),
            ("prompt", json!(" "), PromptEmpty),
            ("prompt", json!("x".repeat(32769)), PromptTooLarge),
            ("prompt", json!("private\0prompt"), PromptControl),
            ("themes", json!([0]), ThemeIndex),
            ("themes", json!([2]), ThemeIndex),
            ("themes", json!([1, 1]), ThemeDuplicate),
        ] {
            let mut v = good.clone();
            v["groups"][0][field] = value;
            check(v, 1, rule, Some(1));
        }
        for (field, value, rule) in [
            ("themes", json!([]), ThemeCount),
            ("themes", json!([""]), ThemeLabel),
            ("themes", json!(["private", "missing"]), ThemeMissing),
            ("groups", json!([]), GroupCount),
        ] {
            let mut v = good.clone();
            v[field] = value;
            check(v, 1, rule, None);
        }
        check(good.clone(), 0, GroupCount, None);
        let mut v = good.clone();
        v["groups"][0]["text"] = json!("x".repeat(1024 * 1024 + 1));
        check(v, 1, TextTooLarge, None);
        let mut v = good;
        v["private_unknown_field"] = json!("private data");
        check(v, 1, JsonSchema, None);
        assert_eq!(
            validate_illustrated_detailed(b"```private```", 1)
                .unwrap_err()
                .rule,
            JsonSyntax
        );
        assert_eq!(
            validate_illustrated_detailed(&vec![b' '; MAX_RESULT_BYTES + 1], 1)
                .unwrap_err()
                .rule,
            ResultTooLarge
        );
    }
}
