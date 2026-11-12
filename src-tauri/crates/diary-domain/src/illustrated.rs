//! Portable illustrated Markdown with a closed, canonical YAML subset.
use crate::authentic::{DiaryDate, valid_uuid};
use pulldown_cmark::{Event, Parser, Tag};

pub const MAX_PHOTOS: usize = 99;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Photo {
    pub seed: u32,
    pub prompt: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: String,
    pub date: DiaryDate,
    pub created_at: String,
    pub source_id: String,
    pub source_revision: u64,
    pub source_sha256: String,
    pub agent_profile: String,
    pub image_engine: String,
    pub trigger: String,
    pub directives: Vec<String>,
    pub theme_count: usize,
    pub photos: Vec<Photo>,
    pub body: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Invalid,
    ImageReference,
    TooLarge,
}
fn quoted(s: &str) -> Result<String, Error> {
    if s.chars()
        .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
        || s.len() > 32768
    {
        return Err(Error::Invalid);
    }
    Ok(format!(
        "\"{}\"",
        s.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\t', "\\t")
    ))
}
fn unquote(s: &str) -> Result<String, Error> {
    let mut out = String::new();
    let mut chars = s
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .ok_or(Error::Invalid)?
        .chars();
    while let Some(c) = chars.next() {
        out.push(match c {
            '\\' => match chars.next() {
                Some('n') => '\n',
                Some('t') => '\t',
                Some(c @ ('"' | '\\')) => c,
                _ => return Err(Error::Invalid),
            },
            '"' => return Err(Error::Invalid),
            c => c,
        });
    }
    Ok(out)
}
impl Entry {
    pub fn validate(&self) -> Result<(), Error> {
        let alias = |s: &str| {
            !s.is_empty()
                && s.len() <= 64
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        };
        if !alias(&self.agent_profile)
            || !alias(&self.image_engine)
            || self.theme_count > 1024
            || self.trigger.len() > 1024
            || self.trigger.contains(['\n', '\r', '\0'])
            || self.directives.len() > 64
            || self
                .directives
                .iter()
                .any(|s| s.trim().is_empty() || s.len() > 4096 || s.contains(['\r', '\0']))
        {
            return Err(Error::Invalid);
        }
        if !valid_uuid(&self.id)
            || !valid_uuid(&self.source_id)
            || self.source_revision == 0
            || self.source_sha256.len() != 64
            || !self
                .source_sha256
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            || self.photos.is_empty()
            || self.photos.len() > MAX_PHOTOS
            || self.theme_count < self.photos.len()
            || self.agent_profile.is_empty()
            || self.image_engine.is_empty()
            || !crate::authentic::valid_instant(&self.created_at)
            || self.body.trim().is_empty()
            || self.body.contains(['\r', '\0'])
        {
            return Err(Error::Invalid);
        }
        if self.body.len() > 1024 * 1024 {
            return Err(Error::TooLarge);
        }
        let mut seen = vec![false; self.photos.len()];
        for event in Parser::new(&self.body) {
            match event {
                Event::Start(Tag::Image { dest_url, .. }) => {
                    let Some(i) = (0..self.photos.len())
                        .find(|i| dest_url.as_ref() == format!("{}/{:02}.png", self.date, i + 1))
                    else {
                        return Err(Error::ImageReference);
                    };
                    if seen[i] {
                        return Err(Error::ImageReference);
                    }
                    seen[i] = true;
                }
                Event::Html(_) | Event::InlineHtml(_) => return Err(Error::ImageReference),
                _ => {}
            }
        }
        if seen.contains(&false) {
            return Err(Error::ImageReference);
        }
        for p in &self.photos {
            if p.prompt.trim().is_empty()
                || p.prompt.len() > 32768
                || p.prompt.contains('\0')
                || (!self.trigger.is_empty() && !p.prompt.starts_with(&self.trigger))
            {
                return Err(Error::Invalid);
            }
        }
        Ok(())
    }
    pub fn render(&self) -> Result<Vec<u8>, Error> {
        self.validate()?;
        let mut s = String::from("---\nschema_version: 1\nasset_type: diary\nkind: illustrated\n");
        for (k, v) in [
            ("id", self.id.as_str()),
            ("diary_date", &self.date.to_string()),
            ("created_at", &self.created_at),
            ("source_authentic_id", &self.source_id),
        ] {
            s.push_str(&format!("{k}: {}\n", quoted(v)?));
        }
        s.push_str(&format!("source_revision: {}\n", self.source_revision));
        for (k, v) in [
            ("source_sha256", &self.source_sha256),
            ("agent_profile", &self.agent_profile),
            ("image_engine", &self.image_engine),
            ("trigger", &self.trigger),
        ] {
            s.push_str(&format!("{k}: {}\n", quoted(v)?));
        }
        s.push_str("prompt_directives:\n");
        for d in &self.directives {
            s.push_str(&format!("  - {}\n", quoted(d)?));
        }
        s.push_str(&format!("theme_count: {}\nphotos:\n", self.theme_count));
        for (i, p) in self.photos.iter().enumerate() {
            s.push_str(&format!(
                "  - file: \"{:02}.png\"\n    seed: {}\n    prompt: {}\n",
                i + 1,
                p.seed,
                quoted(&p.prompt)?
            ));
        }
        s.push_str("---\n");
        s.push_str(&self.body);
        Ok(s.into_bytes())
    }
    pub fn parse(bytes: &[u8], date: DiaryDate) -> Result<Self, Error> {
        if bytes.len() > 5 * 1024 * 1024 {
            return Err(Error::TooLarge);
        }
        let s = std::str::from_utf8(bytes).map_err(|_| Error::Invalid)?;
        let (h, body) = s
            .strip_prefix("---\n")
            .and_then(|s| s.split_once("\n---\n"))
            .ok_or(Error::Invalid)?;
        let lines: Vec<_> = h.lines().collect();
        let mut i = 0;
        fn field<'a>(lines: &[&'a str], i: &mut usize, k: &str) -> Result<&'a str, Error> {
            let v = lines
                .get(*i)
                .and_then(|s| s.strip_prefix(&format!("{k}: ")))
                .ok_or(Error::Invalid)?;
            *i += 1;
            Ok(v)
        }
        if field(&lines, &mut i, "schema_version")? != "1"
            || field(&lines, &mut i, "asset_type")? != "diary"
            || field(&lines, &mut i, "kind")? != "illustrated"
        {
            return Err(Error::Invalid);
        }
        let id = unquote(field(&lines, &mut i, "id")?)?;
        if unquote(field(&lines, &mut i, "diary_date")?)? != date.to_string() {
            return Err(Error::Invalid);
        }
        let created_at = unquote(field(&lines, &mut i, "created_at")?)?;
        let source_id = unquote(field(&lines, &mut i, "source_authentic_id")?)?;
        let source_revision = field(&lines, &mut i, "source_revision")?
            .parse()
            .map_err(|_| Error::Invalid)?;
        let source_sha256 = unquote(field(&lines, &mut i, "source_sha256")?)?;
        let agent_profile = unquote(field(&lines, &mut i, "agent_profile")?)?;
        let image_engine = unquote(field(&lines, &mut i, "image_engine")?)?;
        let trigger = unquote(field(&lines, &mut i, "trigger")?)?;
        if lines.get(i) != Some(&"prompt_directives:") {
            return Err(Error::Invalid);
        }
        i += 1;
        let mut directives = vec![];
        while let Some(v) = lines.get(i).and_then(|s| s.strip_prefix("  - ")) {
            directives.push(unquote(v)?);
            i += 1;
        }
        let theme_count = field(&lines, &mut i, "theme_count")?
            .parse()
            .map_err(|_| Error::Invalid)?;
        if lines.get(i) != Some(&"photos:") {
            return Err(Error::Invalid);
        }
        i += 1;
        let mut photos = vec![];
        while i < lines.len() {
            if field(&lines, &mut i, "  - file")? != format!("\"{:02}.png\"", photos.len() + 1) {
                return Err(Error::Invalid);
            }
            let seed = field(&lines, &mut i, "    seed")?
                .parse()
                .map_err(|_| Error::Invalid)?;
            let prompt = unquote(field(&lines, &mut i, "    prompt")?)?;
            photos.push(Photo { seed, prompt });
        }
        let e = Self {
            id,
            date,
            created_at,
            source_id,
            source_revision,
            source_sha256,
            agent_profile,
            image_engine,
            trigger,
            directives,
            theme_count,
            photos,
            body: body.into(),
        };
        e.validate()?;
        if e.render()? != bytes {
            return Err(Error::Invalid);
        }
        Ok(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    pub fn entry() -> Entry {
        Entry {
            id: "07249746-e280-4d51-a86a-cbcb198ee101".into(),
            date: DiaryDate::parse("2026-11-13").unwrap(),
            created_at: "2026-11-13T00:00:00Z".into(),
            source_id: "07249746-e280-4d51-a86a-cbcb198ee102".into(),
            source_revision: 1,
            source_sha256: "a".repeat(64),
            agent_profile: "test".into(),
            image_engine: "local".into(),
            trigger: "tag".into(),
            directives: vec!["a photo".into()],
            theme_count: 1,
            photos: vec![Photo {
                seed: 1,
                prompt: "tag café \"morning\"\nlight".into(),
            }],
            body: "A walk.\n\n![Walk](2026-11-13/01.png)\n".into(),
        }
    }
    #[test]
    fn canonical_roundtrip_and_reference_rejection() {
        let e = entry();
        let b = e.render().unwrap();
        assert_eq!(Entry::parse(&b, e.date).unwrap(), e);
        for body in [
            "![x](../01.png)",
            "![x](https://example.com/x.png)",
            "<img src=\"x\">",
            "![x][ref]\n\n[ref]: /private/x.png",
            "no photo",
        ] {
            let mut x = e.clone();
            x.body = body.into();
            assert!(x.render().is_err());
        }
        let mut x = b.clone();
        x.extend_from_slice(b"\r");
        assert!(Entry::parse(&x, e.date).is_err());
    }
}
