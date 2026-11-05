//! A bounded BCP-47 subset: language[-script][-region][-variant]*, no extensions or private use.

fn alpha(s: &str) -> bool {
    s.bytes().all(|b| b.is_ascii_alphabetic())
}
fn digits(s: &str) -> bool {
    s.bytes().all(|b| b.is_ascii_digit())
}
/// The canonical tag (`ja_jp` becomes `ja-JP`), or `None` when the input is outside the subset.
pub fn canonical_tag(input: &str) -> Option<String> {
    if input.is_empty() || input.len() > 35 || !input.is_ascii() {
        return None;
    }
    let mut parts = input.split(['-', '_']).peekable();
    let language = parts.next()?;
    if !(2..=3).contains(&language.len()) || !alpha(language) {
        return None;
    }
    let mut out = vec![language.to_ascii_lowercase()];
    if let Some(p) = parts.next_if(|p| p.len() == 4 && alpha(p)) {
        let (head, tail) = p.split_at(1);
        out.push(format!(
            "{}{}",
            head.to_ascii_uppercase(),
            tail.to_ascii_lowercase()
        ));
    }
    if let Some(p) = parts.next_if(|p| (p.len() == 2 && alpha(p)) || (p.len() == 3 && digits(p))) {
        out.push(p.to_ascii_uppercase());
    }
    for p in parts {
        let alnum = p.bytes().all(|b| b.is_ascii_alphanumeric());
        let variant = alnum
            && ((5..=8).contains(&p.len()) || (p.len() == 4 && p.as_bytes()[0].is_ascii_digit()));
        if !variant {
            return None;
        }
        out.push(p.to_ascii_lowercase());
    }
    Some(out.join("-"))
}
