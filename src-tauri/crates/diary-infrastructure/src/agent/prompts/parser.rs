use super::{
    ParsedPromptAsset, PromptTemplate, SESSION,
    renderer::{Piece, pieces},
};
use diary_application::agent::{AgentExecutionSettings, AgentKind};

pub(super) fn parse(
    kind: AgentKind,
    name: &str,
    text: &str,
) -> Result<ParsedPromptAsset, &'static str> {
    if text.len() > 64 * 1024 {
        return Err("asset too large");
    }
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
    if kind.spec().fixed_pins && (model != "default" || reasoning_effort != "not-applicable") {
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
    Ok(ParsedPromptAsset {
        settings: AgentExecutionSettings {
            model,
            reasoning_effort,
        },
        source: text.into(),
        template: PromptTemplate { body },
    })
}
