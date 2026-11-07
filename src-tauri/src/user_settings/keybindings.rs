//! Shortcut registry, canonical form, reserved keys and the effective-binding merge
//! (docs/architecture/keyboard-shortcuts.md). The registry JSON is shared with TypeScript.
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::sync::OnceLock;

#[derive(Deserialize)]
pub struct Action {
    pub id: String,
    pub scope: String,
    pub defaults: Vec<String>,
}
#[derive(Deserialize)]
pub struct Reserved {
    all: Vec<String>,
    app: Vec<String>,
}
#[derive(Deserialize)]
pub struct Registry {
    pub actions: Vec<Action>,
    reserved: Reserved,
    #[cfg(test)]
    pub fixtures: Value,
}
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Issue {
    pub id: String,
    pub code: &'static str,
}

pub fn registry() -> &'static Registry {
    static R: OnceLock<Registry> = OnceLock::new();
    R.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../src/application/actions/registry.json"
        ))
        .expect("registry.json is valid")
    })
}

const MODS: [&str; 4] = ["Control", "Option", "Shift", "Command"];
const NAMED: [&str; 14] = [
    "Backspace",
    "ForwardDelete",
    "Enter",
    "Escape",
    "Tab",
    "Space",
    "ArrowUp",
    "ArrowDown",
    "ArrowLeft",
    "ArrowRight",
    "Home",
    "End",
    "PageUp",
    "PageDown",
];

fn canonical_key(k: &str) -> Option<String> {
    if let Some(n) = NAMED.iter().find(|n| n.eq_ignore_ascii_case(k)) {
        return Some((*n).to_owned());
    }
    let mut chars = k.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if c.is_ascii_alphanumeric() => Some(c.to_ascii_uppercase().to_string()),
        (Some('f' | 'F'), Some(_)) => {
            // At most two digits, matching the TypeScript side.
            let n: u8 = k[1..].parse().ok().filter(|_| k.len() <= 3)?;
            ((1..=12).contains(&n) && k[1..].chars().all(|c| c.is_ascii_digit()))
                .then(|| format!("F{n}"))
        }
        _ => None,
    }
}
/// Modifiers in canonical order, then the key. `None` when malformed.
pub fn canonical(s: &str) -> Option<String> {
    let mut mods = [false; 4];
    let mut key = None;
    for part in s.split('+') {
        if let Some(i) = MODS.iter().position(|m| m.eq_ignore_ascii_case(part)) {
            if std::mem::replace(&mut mods[i], true) {
                return None;
            }
        } else if key.replace(canonical_key(part)?).is_some() {
            return None;
        }
    }
    let mut out: Vec<String> = MODS
        .iter()
        .zip(mods)
        .filter(|(_, on)| *on)
        .map(|(m, _)| (*m).to_owned())
        .collect();
    out.push(key?);
    Some(out.join("+"))
}

/// `Ok(canonical)` or the issue code (`invalid_shortcut`, `reserved`).
pub fn validate(scope: &str, s: &str) -> Result<String, &'static str> {
    if s.len() > 64 {
        return Err("invalid_shortcut");
    }
    let c = canonical(s).ok_or("invalid_shortcut")?;
    let parts: Vec<&str> = c.split('+').collect();
    let (key, mods) = parts.split_last().expect("canonical has a key");
    let has = |m: &str| mods.contains(&m);
    let printable = key.len() == 1 || *key == "Space";
    let deleting = matches!(*key, "Backspace" | "ForwardDelete");
    let r = registry();
    let reserved = r.reserved.all.contains(&c)
        || (scope == "app" && r.reserved.app.contains(&c))
        || key.starts_with("Arrow")
        || (has("Option") && !has("Command") && !has("Control") && printable)
        || (mods == ["Control"] && matches!(*key, "A" | "E" | "K" | "F" | "B" | "N" | "P"))
        || (!(has("Command") || has("Control") || has("Option"))
            && !(scope == "snippetTree" && has("Shift") && mods.len() == 1 && deleting));
    if reserved { Err("reserved") } else { Ok(c) }
}

fn overlap(a: &str, b: &str) -> bool {
    a == b || a == "app" || b == "app"
}

/// Effective bindings in registry order plus issues. `overrides` is the stored object.
pub fn effective(overrides: &Map<String, Value>) -> (Map<String, Value>, Vec<Issue>) {
    let r = registry();
    let mut issues = Vec::new();
    // (action index, shortcut, is_override)
    let mut entries: Vec<(usize, String, bool)> = Vec::new();
    for (id, value) in overrides {
        let Some(i) = r.actions.iter().position(|a| a.id == *id) else {
            issues.push(Issue {
                id: id.clone(),
                code: "unknown_action",
            });
            continue;
        };
        let list = value
            .as_array()
            .filter(|a| a.len() <= 8)
            .map(|a| a.iter().map(Value::as_str).collect::<Vec<_>>());
        let Some(list) = list else {
            issues.push(Issue {
                id: id.clone(),
                code: "invalid_shortcut",
            });
            continue;
        };
        for s in list {
            match s
                .ok_or("invalid_shortcut")
                .and_then(|s| validate(&r.actions[i].scope, s))
            {
                Ok(c) => entries.push((i, c, true)),
                Err(code) => issues.push(Issue {
                    id: id.clone(),
                    code,
                }),
            }
        }
    }
    let overridden = |i: usize| {
        overrides
            .get(&r.actions[i].id)
            .is_some_and(|v| v.as_array().is_some_and(|a| a.len() <= 8))
    };
    for (i, a) in r.actions.iter().enumerate() {
        if !overridden(i) {
            entries.extend(a.defaults.iter().map(|d| (i, d.clone(), false)));
        }
    }
    // Keep insertion order (user order for overrides, registry order for defaults).
    let mut seen = std::collections::HashSet::new();
    entries.retain(|(i, s, _)| seen.insert((*i, s.clone())));
    // An override survives only against defaults; any other collision deactivates both sides.
    let keep: Vec<bool> = entries
        .iter()
        .map(|(i, s, is_override)| {
            let mut colliders = entries.iter().filter(|(j, t, _)| {
                t == s && j != i && overlap(&r.actions[*i].scope, &r.actions[*j].scope)
            });
            colliders.all(|(_, _, other)| *is_override && !other)
        })
        .collect();
    let mut out = Map::new();
    for (i, a) in r.actions.iter().enumerate() {
        let mut shortcuts = Vec::new();
        for (e, k) in entries.iter().zip(&keep).filter(|(e, _)| e.0 == i) {
            if *k {
                shortcuts.push(Value::String(e.1.clone()));
            } else if !issues.iter().any(|x| x.id == a.id && x.code == "conflict") {
                issues.push(Issue {
                    id: a.id.clone(),
                    code: "conflict",
                });
            }
        }
        out.insert(a.id.clone(), Value::Array(shortcuts));
    }
    (out, issues)
}

/// Applies a one-action edit (`{"set": {id: [..] | null}}`) or `{"reset_all": true}`.
/// `None` is an invalid edit. Entries not named stay exactly as stored.
pub fn apply(stored: &Map<String, Value>, change: &Value) -> Option<Map<String, Value>> {
    let object = change.as_object().filter(|o| o.len() == 1)?;
    if object.get("reset_all") == Some(&Value::Bool(true)) {
        return Some(Map::new());
    }
    let set = object
        .get("set")?
        .as_object()
        .filter(|s| !s.is_empty() && s.len() <= 12)?;
    let mut next = stored.clone();
    for (id, value) in set {
        let action = registry().actions.iter().find(|a| a.id == *id)?;
        if value.is_null() {
            next.remove(id);
            continue;
        }
        let list = value.as_array().filter(|a| a.len() <= 8)?;
        let mut clean: Vec<Value> = Vec::new();
        for s in list {
            let c = validate(&action.scope, s.as_str()?).ok()?;
            if !clean.iter().any(|x| x.as_str() == Some(&c)) {
                clean.push(Value::String(c));
            }
        }
        next.insert(id.clone(), Value::Array(clean));
    }
    Some(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn registry_parity() {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        let want: [(&str, &str, Vec<String>); 13] = [
            ("palette.open", "app", s(&["Shift+Command+P"])),
            ("editor.save", "app", s(&["Command+S"])),
            ("authentic.create_empty", "app", s(&[])),
            (
                "source.trash",
                "snippetTree",
                s(&[
                    "Shift+Backspace",
                    "Shift+ForwardDelete",
                    "Command+Backspace",
                    "Command+ForwardDelete",
                ]),
            ),
            ("source.refresh", "app", s(&[])),
            ("generate.start", "app", s(&[])),
            ("ai.activity.open", "app", s(&[])),
            ("ai.run.cancel", "app", s(&[])),
            ("ai.results.open", "app", s(&[])),
            ("ai.results.discard", "app", s(&[])),
            ("export.start", "app", s(&[])),
            ("export.start.notion", "app", s(&[])),
            ("settings.open", "app", s(&[])),
        ];
        let got = &registry().actions;
        assert_eq!(got.len(), want.len());
        for (g, w) in got.iter().zip(&want) {
            assert_eq!(
                (g.id.as_str(), g.scope.as_str(), &g.defaults),
                (w.0, w.1, &w.2)
            );
        }
    }

    #[test]
    fn fixtures_and_factory_defaults() {
        let f = &registry().fixtures;
        for pair in f["canonical"].as_array().unwrap() {
            assert_eq!(
                canonical(pair[0].as_str().unwrap()).as_deref(),
                pair[1].as_str()
            );
        }
        for bad in f["invalid"].as_array().unwrap() {
            assert_eq!(canonical(bad.as_str().unwrap()), None, "{bad}");
        }
        for row in f["validate"].as_array().unwrap() {
            let got = validate(row[0].as_str().unwrap(), row[1].as_str().unwrap());
            let want = row[2].as_str().unwrap();
            assert_eq!(got.as_ref().map_or_else(|e| *e, |_| "ok"), want, "{row}");
        }
        for a in &registry().actions {
            for d in &a.defaults {
                assert_eq!(validate(&a.scope, d).as_deref(), Ok(d.as_str()), "{d}");
            }
        }
        let (bindings, issues) = effective(&Map::new());
        assert!(issues.is_empty());
        let trash = &registry().actions[3];
        assert_eq!(bindings["source.trash"], json!(trash.defaults));
        assert_eq!(canonical("F001"), None);
        assert_eq!(bindings["palette.open"], json!(["Shift+Command+P"]));
    }
}
