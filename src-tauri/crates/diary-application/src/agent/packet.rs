//! Request values: locale, style, generation and chat packets, and the immutable request.
use super::{
    AgentError, AgentErrorCode, MAX_TRANSCRIPT_BYTES, MAX_TURNS, MAX_USER_MESSAGE_BYTES,
    ProfileHandle, SCHEMA_VERSION,
};
use diary_domain::authentic::DiaryDate;

/// BCP-47 shape only; never a registry lookup (shortcut: upgrade if an adapter rejects a valid tag).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Locale(String);
impl Locale {
    pub fn parse(s: &str) -> Result<Self, AgentError> {
        let ok = (2..=35).contains(&s.len())
            && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            && s.split('-').enumerate().all(|(i, t)| {
                (1..=8).contains(&t.len()) && (i > 0 || t.bytes().all(|b| b.is_ascii_alphabetic()))
            });
        if ok {
            Ok(Self(s.to_owned()))
        } else {
            Err(AgentErrorCode::UnsupportedLanguage.into())
        }
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Standard,
    Concise,
    Reflective,
    Chronological,
    Thematic,
}
impl Style {
    /// Deterministic candidate order.
    pub const ALL: [Self; 5] = [
        Self::Standard,
        Self::Concise,
        Self::Reflective,
        Self::Chronological,
        Self::Thematic,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Concise => "concise",
            Self::Reflective => "reflective",
            Self::Chronological => "chronological",
            Self::Thematic => "thematic",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnippetItem {
    pub id: String,
    pub instant_utc: String,
    pub civil_date: String,
    pub offset: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerationPacket {
    style: Style,
    target_date: DiaryDate,
    snippets: Vec<SnippetItem>,
}
impl GenerationPacket {
    /// Rejects an empty selection and any ID that is not the sequential `snippet-N` label.
    pub fn new(
        style: Style,
        target_date: DiaryDate,
        snippets: Vec<SnippetItem>,
    ) -> Result<Self, AgentError> {
        let sequential = snippets
            .iter()
            .enumerate()
            .all(|(i, s)| s.id == format!("snippet-{}", i + 1));
        if snippets.is_empty() || !sequential {
            return Err(AgentErrorCode::Context.into());
        }
        Ok(Self {
            style,
            target_date,
            snippets,
        })
    }
    pub fn style(&self) -> Style {
        self.style
    }
    pub fn target_date(&self) -> DiaryDate {
        self.target_date
    }
    pub fn snippets(&self) -> &[SnippetItem] {
        &self.snippets
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentKind {
    Authentic,
    Source,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatDocument {
    pub kind: DocumentKind,
    pub date: DiaryDate,
    pub unsaved: bool,
    pub body: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Turn {
    pub role: Role,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatPacket {
    document: Option<ChatDocument>,
    transcript: Vec<Turn>,
    user_message: String,
}
impl ChatPacket {
    /// Over-bound input is `context`; nothing is ever truncated.
    pub fn new(
        document: Option<ChatDocument>,
        transcript: Vec<Turn>,
        user_message: String,
    ) -> Result<Self, AgentError> {
        let bytes: usize = transcript.iter().map(|t| t.text.len()).sum();
        if user_message.len() > MAX_USER_MESSAGE_BYTES
            || transcript.len() > MAX_TURNS
            || bytes > MAX_TRANSCRIPT_BYTES
        {
            return Err(AgentErrorCode::Context.into());
        }
        Ok(Self {
            document,
            transcript,
            user_message,
        })
    }
    pub fn document(&self) -> Option<&ChatDocument> {
        self.document.as_ref()
    }
    pub fn transcript(&self) -> &[Turn] {
        &self.transcript
    }
    pub fn user_message(&self) -> &str {
        &self.user_message
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    Chat(ChatPacket),
    Candidate {
        index: u8,
        count: u8,
        packet: GenerationPacket,
    },
}

/// Immutable after construction (private fields).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRequest {
    profile: ProfileHandle,
    locale: Locale,
    mode: Mode,
}
impl AgentRequest {
    pub fn new(profile: ProfileHandle, locale: Locale, mode: Mode) -> Self {
        Self {
            profile,
            locale,
            mode,
        }
    }
    pub fn schema_version(&self) -> u32 {
        SCHEMA_VERSION
    }
    pub fn profile(&self) -> &ProfileHandle {
        &self.profile
    }
    pub fn locale(&self) -> &Locale {
        &self.locale
    }
    pub fn mode(&self) -> &Mode {
        &self.mode
    }
}
