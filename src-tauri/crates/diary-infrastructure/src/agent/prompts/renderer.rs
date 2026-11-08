use super::PromptTemplate;
use crate::agent::packet::encode_packet;
use diary_application::agent::{AgentError, AgentRequest, Locale, PromptRenderer, RenderedPrompt};

pub(super) enum Piece<'a> {
    Text(&'a str),
    Locale,
    Data,
}

/// Splits on `{{locale}}` and `{{data}}`; any other `{{` is malformed.
pub(super) fn pieces(s: &str) -> Result<Vec<Piece<'_>>, &'static str> {
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

impl PromptTemplate {
    /// Single pass: inserted values are never rescanned, so diary text with `{{...}}` stays verbatim.
    pub(super) fn render(&self, locale: &Locale, data: Option<&str>) -> String {
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

pub(super) struct TemplateRenderer {
    pub task: PromptTemplate,
    pub session: Option<PromptTemplate>,
}
impl PromptRenderer for TemplateRenderer {
    fn retained_bytes(&self) -> usize {
        self.task.body.len() + self.session.as_ref().map_or(0, |part| part.body.len())
    }
    fn render(&self, request: &AgentRequest) -> Result<RenderedPrompt, AgentError> {
        let packet = encode_packet(request.mode())?;
        Ok(RenderedPrompt {
            task: self.task.render(request.locale(), Some(&packet)),
            session: self
                .session
                .as_ref()
                .map(|part| part.render(request.locale(), None)),
        })
    }
}
