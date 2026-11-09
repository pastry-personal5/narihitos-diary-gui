//! Bounded conversation ownership shared by transports. IDs are opaque outside this module.
use super::Chat;
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, Default)]
pub enum Purpose {
    #[default]
    Chat,
    ParagraphReview,
}
impl Purpose {
    pub fn conversation_id(self, unique: &str) -> String {
        match self {
            Self::Chat => unique.to_owned(),
            Self::ParagraphReview => format!("paragraph-{unique}"),
        }
    }
}
#[derive(Default)]
pub struct Conversations {
    chat: Mutex<Option<Chat>>,
    paragraph_review: Mutex<Option<Chat>>,
}
impl Conversations {
    pub fn slot(&self, purpose: Purpose) -> &Mutex<Option<Chat>> {
        match purpose {
            Purpose::Chat => &self.chat,
            Purpose::ParagraphReview => &self.paragraph_review,
        }
    }
    pub fn for_id(&self, id: &str) -> &Mutex<Option<Chat>> {
        self.slot(if id.starts_with("paragraph-") {
            Purpose::ParagraphReview
        } else {
            Purpose::Chat
        })
    }
    /// Visits held conversations under their slot lock; callbacks must not reenter this registry.
    pub fn each(&self, mut action: impl FnMut(&Chat)) {
        for slot in [&self.chat, &self.paragraph_review] {
            if let Some(chat) = slot.lock().expect("conversation poisoned").as_ref() {
                action(chat);
            }
        }
    }
    pub fn is_empty(&self) -> bool {
        let mut empty = true;
        self.each(|_| empty = false);
        empty
    }
    pub fn clear(&self) {
        for slot in [&self.chat, &self.paragraph_review] {
            if let Some(chat) = slot.lock().expect("conversation poisoned").take() {
                chat.cancel();
            }
        }
    }
}
