//! Prompt assets embedded with `include_str!`. The loader takes a table, never a path, so it cannot
//! read the working directory; each asset parses on its own, so one bad file disables only its action.
use diary_application::agent::{Action, AgentKind, AssetState, Capability, Locale, capability};
use std::collections::HashMap;

pub type Table<'a> = [(AgentKind, &'a str, &'a str)];

macro_rules! asset {
    ($kind:expr, $dir:literal, $name:literal) => {
        (
            $kind,
            $name,
            include_str!(concat!(
                "../../../../../assets/prompts/",
                $dir,
                "/",
                $name,
                ".md"
            )),
        )
    };
}

pub const EMBEDDED: &Table<'static> = &[
    asset!(AgentKind::Codex, "codex", "generate-candidate"),
    asset!(AgentKind::Codex, "codex", "chat"),
    asset!(AgentKind::Claude, "claude", "generate-candidate"),
    asset!(AgentKind::Claude, "claude", "chat"),
    asset!(AgentKind::Qwen, "qwen", "generate-candidate"),
    asset!(AgentKind::Qwen, "qwen", "chat"),
    asset!(
        AgentKind::AppleFoundationModels,
        "apple-foundation-models",
        "generate-candidate"
    ),
    asset!(
        AgentKind::AppleFoundationModels,
        "apple-foundation-models",
        "chat"
    ),
    asset!(
        AgentKind::AppleFoundationModels,
        "apple-foundation-models",
        "session-instructions"
    ),
];

pub const GENERATE: &str = "generate-candidate";
pub const CHAT: &str = "chat";
pub const SESSION: &str = "session-instructions";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prompt {
    pub provider: String,
    /// Passed through unchanged; the loader never claims the value is supported.
    pub model: String,
    pub reasoning_effort: String,
    body: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetReason {
    Missing,
    Malformed(&'static str),
}
/// Names the asset only; carries no asset text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssetError {
    pub kind: AgentKind,
    pub name: &'static str,
    pub reason: AssetReason,
}

enum Piece<'a> {
    Text(&'a str),
    Locale,
    Data,
}

/// Splits on `{{locale}}` and `{{data}}`; any other `{{` is malformed.
fn pieces(s: &str) -> Result<Vec<Piece<'_>>, &'static str> {
    let (mut out, mut rest) = (Vec::new(), s);
    while let Some(i) = rest.find("{{") {
        out.push(Piece::Text(&rest[..i]));
        let after = &rest[i + 2..];
        if let Some(r) = after.strip_prefix("locale}}") {
            out.push(Piece::Locale);
            rest = r;
        } else if let Some(r) = after.strip_prefix("data}}") {
            out.push(Piece::Data);
            rest = r;
        } else {
            return Err("unknown placeholder");
        }
    }
    out.push(Piece::Text(rest));
    Ok(out)
}

fn parse(kind: AgentKind, name: &str, text: &str) -> Result<Prompt, &'static str> {
    let rest = text.strip_prefix("---\n").ok_or("front matter")?;
    let (fm, body) = rest.split_once("\n---\n").ok_or("front matter")?;
    let (mut provider, mut model, mut effort) = (None, None, None);
    for line in fm.lines() {
        let (k, v) = line.split_once(": ").ok_or("front matter line")?;
        let v = v.trim();
        let slot = match k {
            "provider" => &mut provider,
            "model" => &mut model,
            "reasoning_effort" => &mut effort,
            _ => return Err("unknown front matter key"),
        };
        if v.is_empty() || slot.replace(v.to_owned()).is_some() {
            return Err("empty or duplicate front matter value");
        }
    }
    let (Some(provider), Some(model), Some(reasoning_effort)) = (provider, model, effort) else {
        return Err("missing front matter key");
    };
    if provider != kind.asset_dir() {
        return Err("provider does not match directory");
    }
    if kind == AgentKind::AppleFoundationModels
        && (model != "default" || reasoning_effort != "not-applicable")
    {
        return Err("apple model and effort must be default / not-applicable");
    }
    let body = body.trim_start_matches('\n');
    let body = body.strip_suffix('\n').unwrap_or(body).to_owned();
    let (mut locale, mut data) = (0, 0);
    for p in pieces(&body)? {
        match p {
            Piece::Locale => locale += 1,
            Piece::Data => data += 1,
            Piece::Text(_) => {}
        }
    }
    let ok = if name == SESSION {
        locale >= 1 && data == 0
    } else {
        locale >= 1 && data == 1
    };
    if !ok {
        return Err("required placeholders");
    }
    Ok(Prompt {
        provider,
        model,
        reasoning_effort,
        body,
    })
}

impl Prompt {
    /// Single pass: inserted values are never rescanned, so diary text with `{{...}}` stays verbatim.
    pub fn render(&self, locale: &Locale, data: Option<&str>) -> String {
        let mut out = String::with_capacity(self.body.len() + data.map_or(0, str::len));
        for p in pieces(&self.body).expect("validated at load") {
            match p {
                Piece::Text(t) => out.push_str(t),
                Piece::Locale => out.push_str(locale.as_str()),
                Piece::Data => out.push_str(data.unwrap_or_default()),
            }
        }
        out
    }
}

pub struct PromptAssets {
    map: HashMap<(AgentKind, &'static str), Result<Prompt, &'static str>>,
}

fn known(name: &str) -> Option<&'static str> {
    [GENERATE, CHAT, SESSION].into_iter().find(|n| *n == name)
}

impl PromptAssets {
    pub fn embedded() -> Self {
        Self::from_table(EMBEDDED)
    }
    pub fn from_table(table: &Table) -> Self {
        // An unknown asset name is not loadable by any action; ignore it.
        let map = table
            .iter()
            .filter_map(|&(kind, name, text)| {
                known(name).map(|n| ((kind, n), parse(kind, n, text)))
            })
            .collect();
        Self { map }
    }
    pub fn get(&self, kind: AgentKind, name: &'static str) -> Result<&Prompt, AssetError> {
        match self.map.get(&(kind, name)) {
            Some(Ok(p)) => Ok(p),
            Some(Err(r)) => Err(AssetError {
                kind,
                name,
                reason: AssetReason::Malformed(r),
            }),
            None => Err(AssetError {
                kind,
                name,
                reason: AssetReason::Missing,
            }),
        }
    }
    pub fn required(kind: AgentKind, action: Action) -> Vec<&'static str> {
        let task = if action == Action::Chat {
            CHAT
        } else {
            GENERATE
        };
        let mut v = vec![task];
        if kind == AgentKind::AppleFoundationModels {
            v.push(SESSION);
        }
        v
    }
    pub fn state(&self, kind: AgentKind, action: Action) -> Vec<AssetState> {
        Self::required(kind, action)
            .into_iter()
            .map(|n| match self.get(kind, n) {
                Ok(_) => AssetState::Valid,
                Err(AssetError {
                    reason: AssetReason::Missing,
                    ..
                }) => AssetState::Missing,
                Err(_) => AssetState::Invalid,
            })
            .collect()
    }
    pub fn capability(
        &self,
        kind: AgentKind,
        action: Action,
        adapter_registered: bool,
    ) -> Capability {
        capability(&self.state(kind, action), adapter_registered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use diary_application::agent::CapabilityReason::*;

    const GOOD: &str =
        "---\nprovider: codex\nmodel: m\nreasoning_effort: e\n---\n\nHi {{locale}} {{data}}\n";
    fn load(kind: AgentKind, text: &str) -> Result<Prompt, &'static str> {
        parse(kind, CHAT, text)
    }
    fn en() -> Locale {
        Locale::parse("en").unwrap()
    }

    #[test]
    fn front_matter_is_parsed() {
        let p = load(AgentKind::Codex, GOOD).unwrap();
        assert_eq!(
            (
                p.provider.as_str(),
                p.model.as_str(),
                p.reasoning_effort.as_str()
            ),
            ("codex", "m", "e")
        );
        assert_eq!(p.render(&en(), Some("D")), "Hi en D"); // trailing newline dropped
        for bad in [
            "no front matter {{locale}}{{data}}",
            "---\nprovider: codex\nmodel: m\n---\n{{locale}}{{data}}",
            "---\nprovider: codex\nprovider: codex\nmodel: m\nreasoning_effort: e\n---\n{{locale}}{{data}}",
            "---\nprovider: codex\nmodel: m\nreasoning_effort: e\nextra: 1\n---\n{{locale}}{{data}}",
            "---\nprovider: codex\nmodel: \nreasoning_effort: e\n---\n{{locale}}{{data}}",
        ] {
            assert!(load(AgentKind::Codex, bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn provider_must_match_directory() {
        assert!(load(AgentKind::Claude, GOOD).is_err());
    }

    #[test]
    fn apple_requires_default_and_not_applicable() {
        let apple = |m: &str, e: &str| {
            format!(
                "---\nprovider: apple-foundation-models\nmodel: {m}\nreasoning_effort: {e}\n---\n{{{{locale}}}}{{{{data}}}}"
            )
        };
        let k = AgentKind::AppleFoundationModels;
        assert!(load(k, &apple("default", "not-applicable")).is_ok());
        assert!(load(k, &apple("gpt", "not-applicable")).is_err());
        assert!(load(k, &apple("default", "medium")).is_err());
    }

    #[test]
    fn cli_model_and_effort_pass_through_unchanged() {
        let t = GOOD
            .replace("model: m", "model: any-name 1")
            .replace("effort: e", "effort: whatever");
        let p = load(AgentKind::Codex, &t).unwrap();
        assert_eq!(
            (p.model.as_str(), p.reasoning_effort.as_str()),
            ("any-name 1", "whatever")
        );
    }

    #[test]
    fn unknown_placeholder_is_malformed() {
        for body in [
            "{{style}} {{locale}} {{data}}",
            "{{ {{locale}} {{data}}",
            "{{locale}} {{data}} {{",
        ] {
            let t = format!("---\nprovider: codex\nmodel: m\nreasoning_effort: e\n---\n{body}");
            assert!(load(AgentKind::Codex, &t).is_err(), "{body}");
        }
    }

    #[test]
    fn required_placeholders_must_appear_once() {
        let t = |b: &str| format!("---\nprovider: codex\nmodel: m\nreasoning_effort: e\n---\n{b}");
        assert!(load(AgentKind::Codex, &t("{{locale}}")).is_err()); // no data
        assert!(load(AgentKind::Codex, &t("{{data}}")).is_err()); // no locale
        assert!(load(AgentKind::Codex, &t("{{locale}}{{data}}{{data}}")).is_err());
        assert!(load(AgentKind::Codex, &t("{{locale}}{{locale}}{{data}}")).is_ok());
        assert!(parse(AgentKind::Codex, SESSION, &t("{{locale}} x")).is_ok());
        assert!(parse(AgentKind::Codex, SESSION, &t("{{locale}}{{data}}")).is_err());
    }

    #[test]
    fn diary_text_with_placeholders_is_not_expanded() {
        let p = load(AgentKind::Codex, GOOD).unwrap();
        assert_eq!(
            p.render(&en(), Some("{{data}} {{locale}}")),
            "Hi en {{data}} {{locale}}"
        );
    }

    #[test]
    fn missing_asset_names_the_action() {
        let a = PromptAssets::from_table(&[]);
        let e = a.get(AgentKind::Codex, CHAT).unwrap_err();
        assert_eq!(
            (e.kind, e.name, e.reason),
            (AgentKind::Codex, CHAT, AssetReason::Missing)
        );
    }

    #[test]
    fn malformed_asset_disables_only_its_action() {
        let a = PromptAssets::from_table(&[
            (AgentKind::Codex, CHAT, "garbage"),
            (AgentKind::Codex, GENERATE, GOOD),
        ]);
        assert_eq!(
            a.capability(AgentKind::Codex, Action::Chat, true),
            Capability::Unavailable(AssetsInvalid)
        );
        assert_eq!(
            a.capability(AgentKind::Codex, Action::Generate, true),
            Capability::Ready
        );
    }

    #[test]
    fn no_fallback_to_another_agent_type() {
        let a = PromptAssets::from_table(&[(AgentKind::Codex, CHAT, GOOD)]);
        assert_eq!(
            a.get(AgentKind::Claude, CHAT).unwrap_err().reason,
            AssetReason::Missing
        );
    }

    #[test]
    fn capabilities_follow_assets_and_adapter() {
        let a = PromptAssets::embedded();
        for action in [Action::Chat, Action::Generate] {
            for k in [
                AgentKind::Codex,
                AgentKind::Claude,
                AgentKind::Qwen,
                AgentKind::AppleFoundationModels,
            ] {
                assert_eq!(
                    a.capability(k, action, false),
                    Capability::Unavailable(AdapterUnimplemented)
                );
                // Ready is reachable only with a fake registration.
                assert_eq!(a.capability(k, action, true), Capability::Ready);
            }
        }
    }

    #[test]
    fn embedded_table_equals_disk_and_registers_every_file() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../assets/prompts");
        let mut disk = Vec::new();
        for d in std::fs::read_dir(root)
            .unwrap()
            .flatten()
            .filter(|d| d.path().is_dir())
        {
            for f in std::fs::read_dir(d.path()).unwrap().flatten() {
                if f.file_name().to_string_lossy().starts_with('.') {
                    continue; // Finder's .DS_Store is not an asset
                }
                let name = f.path().file_stem().unwrap().to_str().unwrap().to_owned();
                disk.push((
                    d.file_name().into_string().unwrap(),
                    name,
                    std::fs::read_to_string(f.path()).unwrap(),
                ));
            }
        }
        let mut table: Vec<_> = EMBEDDED
            .iter()
            .map(|(k, n, t)| (k.asset_dir().to_owned(), n.to_string(), t.to_string()))
            .collect();
        disk.sort();
        table.sort();
        assert_eq!(table, disk);
    }

    fn real(kind: AgentKind, name: &'static str) -> Prompt {
        PromptAssets::embedded().get(kind, name).unwrap().clone()
    }

    #[test]
    fn generate_asset_matches_schema_and_packet() {
        for k in [
            AgentKind::Codex,
            AgentKind::Claude,
            AgentKind::Qwen,
            AgentKind::AppleFoundationModels,
        ] {
            let b = real(k, GENERATE).body;
            for want in [
                "draft_markdown",
                "claim_support",
                "snippet_ids",
                "claim",
                "warnings",
                "snippets",
                "target_date",
                "civil_date",
                "instant_utc",
                "snippet-1",
            ] {
                assert!(b.contains(want), "{k:?} {want}");
            }
            for gone in [
                "reviewed_memory",
                "reflection_ranges",
                "prior_draft",
                "language",
            ] {
                assert!(!b.contains(gone), "{k:?} {gone}");
            }
        }
        let c = real(AgentKind::Codex, GENERATE);
        assert_eq!(
            (c.model.as_str(), c.reasoning_effort.as_str()),
            ("gpt-5.6-luna", "medium")
        );
        let c = real(AgentKind::Claude, GENERATE);
        assert_eq!(
            (c.model.as_str(), c.reasoning_effort.as_str()),
            ("claude-sonnet-4-6", "low")
        );
        let c = real(AgentKind::Qwen, GENERATE);
        // Qwen's --json-schema errors on plain-text answers; the asset must name its tool.
        assert!(c.body.contains("`structured_output`"));
        assert_eq!(
            (c.model.as_str(), c.reasoning_effort.as_str()),
            ("qwen3.6:35b-a3b", "not-applicable")
        );
    }

    #[test]
    fn generate_asset_names_all_five_styles() {
        for k in [
            AgentKind::Codex,
            AgentKind::Claude,
            AgentKind::Qwen,
            AgentKind::AppleFoundationModels,
        ] {
            let b = real(k, GENERATE).body;
            for s in diary_application::agent::Style::ALL {
                assert!(b.contains(&format!("`{}`", s.id())), "{k:?} {}", s.id());
            }
        }
    }

    #[test]
    fn chat_asset_matches_packet() {
        for k in [
            AgentKind::Codex,
            AgentKind::Claude,
            AgentKind::Qwen,
            AgentKind::AppleFoundationModels,
        ] {
            let b = real(k, CHAT).body;
            for want in ["document", "transcript", "user_message", "unsaved", "kind"] {
                assert!(b.contains(want), "{k:?} {want}");
            }
            for gone in ["reviewed_memory", "reflection_ranges", "prior_draft"] {
                assert!(!b.contains(gone));
            }
        }
    }
}
