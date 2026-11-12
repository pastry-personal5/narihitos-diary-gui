//! Illustrated creation policy and ports. One caller owns admission through publication.
use crate::{
    agent::*,
    authentic::{AuthenticStore, Loaded},
    image::{Registry, RenderRequest},
};
use diary_domain::{
    authentic::DiaryDate,
    illustrated::{Entry, Photo},
};
use std::{path::PathBuf, sync::Arc};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Invalid,
    NotFound,
    Stale,
    Busy,
    Unavailable,
    ProfileUnavailable,
    EngineUnavailable,
    SourceBusy,
    Agent,
    Render,
    Cancelled,
    Storage,
    ConfirmationRequired,
}
pub trait Store: Send + Sync {
    fn list(&self) -> Result<Vec<DiaryDate>, Error>;
    fn read(&self, date: DiaryDate) -> Result<Option<Entry>, Error>;
    fn photo(&self, date: DiaryDate, version: &str, n: usize) -> Result<Vec<u8>, Error>;
    fn publish(&self, entry: &Entry, photos: &[PathBuf], cancel: &CancelToken)
    -> Result<(), Error>;
    fn delete(&self, date: DiaryDate, version: &str, confirmed: bool) -> Result<(), Error>;
}
#[derive(Clone, Default, PartialEq, Eq)]
pub struct PromptSettings {
    pub trigger: String,
    pub directives: Vec<String>,
}
impl PromptSettings {
    pub fn validate(&self) -> bool {
        self.trigger.len() <= 1024
            && !self.trigger.contains(['\0', '\r', '\n'])
            && self.directives.len() <= 64
            && self
                .directives
                .iter()
                .all(|s| !s.trim().is_empty() && s.len() <= 4096 && !s.contains(['\0', '\r']))
    }
    pub fn compose(&self, scene: &str) -> Result<String, Error> {
        let scene = scene.trim();
        if scene.is_empty() || scene.contains('\0') || !self.validate() {
            return Err(Error::Invalid);
        }
        let mut scene = scene;
        if !self.trigger.is_empty() {
            while let Some(rest) = scene
                .strip_prefix(&self.trigger)
                .filter(|rest| rest.is_empty() || rest.starts_with([' ', ',', '\n']))
            {
                scene = rest.trim_start_matches([',', ' ', '\n']);
            }
        }
        if scene.is_empty() {
            return Err(Error::Invalid);
        }
        let prompt = if self.trigger.is_empty() {
            scene.into()
        } else {
            format!("{} {scene}", self.trigger)
        };
        if prompt.len() > 32768 {
            return Err(Error::Invalid);
        }
        Ok(prompt)
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct Packet {
    pub body: String,
    pub date: DiaryDate,
    pub settings: PromptSettings,
    pub max_photos: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    pub themes: Vec<usize>,
    pub text: String,
    pub alt: String,
    pub prompt: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    pub themes: Vec<String>,
    pub groups: Vec<Group>,
}
impl Plan {
    pub fn validate(&self, cap: usize) -> Result<(), Error> {
        if self.themes.is_empty()
            || self.themes.len() > 1024
            || self.groups.is_empty()
            || self.groups.len() > cap
            || self.groups.len() > 99
        {
            return Err(Error::Invalid);
        }
        let mut seen = vec![false; self.themes.len()];
        for theme in &self.themes {
            if theme.trim().is_empty() || theme.len() > 4096 {
                return Err(Error::Invalid);
            }
        }
        let mut bytes = 0;
        for g in &self.groups {
            bytes += g.text.len();
            if g.themes.is_empty()
                || g.text.trim().is_empty()
                || g.text.contains(['\0', '\r'])
                || g.text.contains("![")
                || g.text.contains('<')
                || g.alt.is_empty()
                || g.alt.len() > 1024
                || g.alt.contains(['[', ']', '\n', '\r', '\\', '\0'])
                || g.prompt.trim().is_empty()
                || g.prompt.len() > 32768
                || g.prompt.contains('\0')
            {
                return Err(Error::Invalid);
            }
            for &i in &g.themes {
                if i == 0 || i > seen.len() || seen[i - 1] {
                    return Err(Error::Invalid);
                }
                seen[i - 1] = true;
            }
        }
        if seen.contains(&false) || bytes > 1024 * 1024 {
            return Err(Error::Invalid);
        }
        Ok(())
    }
}
pub struct Creation<'a> {
    pub authentic: &'a dyn AuthenticStore,
    pub store: &'a dyn Store,
    pub runner: &'a dyn AgentRunner,
    pub images: &'a Registry,
    pub profile: ProfileHandle,
    pub locale: Locale,
    pub engine: String,
    pub settings: PromptSettings,
    pub max_photos: usize,
    pub date: DiaryDate,
    pub id: String,
    pub created_at: String,
    pub cancel: CancelToken,
}
struct Quiet;
impl EventSink for Quiet {
    fn event(&mut self, _: AgentEvent) {}
}
impl Creation<'_> {
    pub fn execute(
        self,
        seed: &mut dyn FnMut() -> u32,
        progress: &mut dyn FnMut(usize, usize),
    ) -> Result<(), Error> {
        if self.cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        if !(1..=99).contains(&self.max_photos) || !self.settings.validate() {
            return Err(Error::Invalid);
        }
        let Loaded::File {
            revision,
            header,
            body,
        } = self.authentic.capture_saved(&self.date).map_err(|e| {
            if e == crate::authentic::StoreError::Reserved {
                Error::SourceBusy
            } else {
                Error::Unavailable
            }
        })?
        else {
            return Err(Error::NotFound);
        };
        if self.runner.check(&self.profile).status != ProfileStatus::Ready {
            return Err(Error::ProfileUnavailable);
        }
        if !matches!(self.images.check(&self.engine),Ok(Some(v)) if v.status==crate::image::Availability::Ready)
        {
            return Err(Error::EngineUnavailable);
        }
        let packet = Packet {
            body,
            date: self.date,
            settings: self.settings.clone(),
            max_photos: self.max_photos,
        };
        progress(0, 0);
        let output = self
            .runner
            .run(
                AgentRequest::new(self.profile.clone(), self.locale, Mode::Illustrated(packet)),
                &mut Quiet,
                &self.cancel,
            )
            .map_err(|_| {
                if self.cancel.is_cancelled() {
                    Error::Cancelled
                } else {
                    Error::Agent
                }
            })?;
        let AgentOutput::Illustrated(plan) = output else {
            return Err(Error::Agent);
        };
        plan.validate(self.max_photos)?;
        // Validate the complete durable representation before starting expensive renders.
        // Markdown blocks can consume appended photo slots, and slot overhead counts
        // toward the body limit even when the agent's text alone fits.
        let mut body = String::new();
        let mut photos = Vec::new();
        for (i, g) in plan.groups.iter().enumerate() {
            body.push_str(&format!(
                "{}\n\n![{}]({}/{:02}.png)\n\n",
                g.text.trim(),
                g.alt,
                self.date,
                i + 1
            ));
            photos.push(Photo {
                seed: 0,
                prompt: self.settings.compose(&g.prompt)?,
            });
        }
        let mut entry = Entry {
            id: self.id,
            date: self.date,
            created_at: self.created_at,
            source_id: header.id,
            source_revision: revision.sequence,
            source_sha256: revision.sha256,
            agent_profile: self.profile.alias,
            image_engine: self.engine,
            trigger: self.settings.trigger,
            directives: self.settings.directives,
            theme_count: plan.themes.len(),
            photos,
            body,
        };
        entry.render().map_err(|_| Error::Invalid)?;
        let mut rendered = Vec::new();
        let total = entry.photos.len();
        for (i, photo) in entry.photos.iter_mut().enumerate() {
            if self.cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            progress(i + 1, total);
            let image = self
                .images
                .render(
                    &entry.image_engine,
                    &RenderRequest {
                        prompt: photo.prompt.clone(),
                        seed: seed(),
                        cancel: self.cancel.clone(),
                    },
                )
                .map_err(|_| {
                    if self.cancel.is_cancelled() {
                        Error::Cancelled
                    } else {
                        Error::Render
                    }
                })?;
            photo.seed = image.seed;
            rendered.push(image);
        }
        self.store.publish(
            &entry,
            &rendered.iter().map(|r| r.path.clone()).collect::<Vec<_>>(),
            &self.cancel,
        )
    }
}
pub type SharedStore = Arc<dyn Store>;

#[derive(Clone)]
pub struct Settings {
    pub engine: Option<String>,
    pub max_photos: usize,
}
pub struct JobProgress {
    pub session: String,
    pub date: DiaryDate,
    pub run: Run,
    pub photo: usize,
    pub total: usize,
    pub terminal: bool,
    pub error: Option<Error>,
}
impl JobProgress {
    pub fn new(session: String, date: DiaryDate) -> Self {
        Self {
            session,
            date,
            run: Run::new(),
            photo: 0,
            total: 0,
            terminal: false,
            error: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_engine_trigger_and_directives_are_independent() {
        let a = PromptSettings {
            trigger: "v02realphoto".into(),
            directives: vec!["photo of a woman in her early 20s".into()],
        };
        let b = PromptSettings {
            trigger: "watercolor".into(),
            directives: vec!["landscape".into()],
        };
        assert_eq!(
            a.compose("v02realphoto morning light").unwrap(),
            "v02realphoto morning light"
        );
        assert_eq!(
            b.compose("a quiet hill").unwrap(),
            "watercolor a quiet hill"
        );
        assert_eq!(
            PromptSettings::default().compose("a quiet hill").unwrap(),
            "a quiet hill"
        );
        assert!(a.compose(&"x".repeat(32768)).is_err());
        assert!(a.compose("\0").is_err());
    }
}
