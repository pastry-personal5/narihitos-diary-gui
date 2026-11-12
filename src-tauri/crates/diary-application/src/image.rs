//! Standalone image generation. Admission is process-wide; engines own disposable results.
use crate::agent::CancelToken;
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineError {
    Unavailable,
    Busy,
    InvalidRequest,
    Failed,
    Timeout,
    Cancelled,
    InvalidOutput,
    Io,
}
pub struct RenderRequest {
    pub prompt: String,
    pub seed: u32,
    pub cancel: CancelToken,
}
impl RenderRequest {
    pub fn validate(&self) -> Result<(), EngineError> {
        if self.cancel.is_cancelled() {
            return Err(EngineError::Cancelled);
        }
        if self.prompt.trim().is_empty() || self.prompt.len() > 32768 || self.prompt.contains('\0')
        {
            return Err(EngineError::InvalidRequest);
        }
        Ok(())
    }
}
/// Owns the private result directory. Drop disposes it; adopt by copying before drop.
pub struct Rendered {
    pub path: PathBuf,
    pub bytes: u64,
    pub width: u32,
    pub height: u32,
    pub seed: u32,
    pub staging: PathBuf,
}
impl Drop for Rendered {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.staging);
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    Unchecked,
    Ready,
    Unavailable,
    Busy,
}
#[derive(Clone)]
pub struct EngineView {
    pub id: String,
    pub label: String,
    pub status: Availability,
}
pub trait ImageEngine: Send + Sync {
    fn prompt_settings(&self) -> crate::illustrated::PromptSettings {
        Default::default()
    }
    fn check(&self) -> Availability;
    fn render(&self, request: &RenderRequest) -> Result<Rendered, EngineError>;
}
struct Entry {
    label: String,
    engine: Arc<dyn ImageEngine>,
    status: Availability,
}
static RENDERING: AtomicBool = AtomicBool::new(false);
struct Slot;
impl Drop for Slot {
    fn drop(&mut self) {
        RENDERING.store(false, Ordering::SeqCst);
    }
}
struct BusyView<'a> {
    views: &'a Mutex<Vec<EngineView>>,
    id: String,
    previous: Availability,
}
impl Drop for BusyView<'_> {
    fn drop(&mut self) {
        if let Some(v) = self
            .views
            .lock()
            .unwrap()
            .iter_mut()
            .find(|v| v.id == self.id)
        {
            v.status = self.previous;
        }
    }
}
#[derive(Default)]
pub struct Registry {
    views: Mutex<Vec<EngineView>>,
    entries: Mutex<BTreeMap<String, Entry>>,
}
impl Registry {
    /// Holds the same lock as render/probe for an atomic configuration replacement.
    pub fn replace(
        &self,
        engines: Vec<(String, String, Arc<dyn ImageEngine>)>,
    ) -> Result<(), EngineError> {
        let mut entries = self.entries.try_lock().map_err(|_| EngineError::Busy)?;
        *entries = engines
            .into_iter()
            .map(|(id, label, engine)| {
                (
                    id,
                    Entry {
                        label,
                        engine,
                        status: Availability::Unchecked,
                    },
                )
            })
            .collect();
        *self.views.lock().unwrap() = entries
            .iter()
            .map(|(id, e)| EngineView {
                id: id.clone(),
                label: e.label.clone(),
                status: e.status,
            })
            .collect();
        Ok(())
    }
    pub fn prompt_settings(
        &self,
        id: &str,
    ) -> Result<crate::illustrated::PromptSettings, EngineError> {
        let entries = self.entries.try_lock().map_err(|_| EngineError::Busy)?;
        entries
            .get(id)
            .map(|e| e.engine.prompt_settings())
            .ok_or(EngineError::Unavailable)
    }
    pub fn list(&self) -> Vec<EngineView> {
        self.views.lock().unwrap().clone()
    }
    pub fn check(&self, id: &str) -> Result<Option<EngineView>, EngineError> {
        let mut entries = self.entries.try_lock().map_err(|_| EngineError::Busy)?;
        let Some(e) = entries.get_mut(id) else {
            return Ok(None);
        };
        e.status = e.engine.check();
        let v = EngineView {
            id: id.into(),
            label: e.label.clone(),
            status: e.status,
        };
        if let Some(cached) = self.views.lock().unwrap().iter_mut().find(|v| v.id == id) {
            *cached = v.clone();
        }
        Ok(Some(v))
    }
    pub fn render(&self, id: &str, request: &RenderRequest) -> Result<Rendered, EngineError> {
        request.validate()?;
        let entries = self.entries.try_lock().map_err(|_| EngineError::Busy)?;
        let e = entries.get(id).ok_or(EngineError::Unavailable)?;
        RENDERING
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| EngineError::Busy)?;
        let _slot = Slot;
        let previous = {
            let mut views = self.views.lock().unwrap();
            let view = views.iter_mut().find(|v| v.id == id).unwrap();
            let previous = view.status;
            view.status = Availability::Busy;
            previous
        };
        let _view = BusyView {
            views: &self.views,
            id: id.into(),
            previous,
        };
        request.validate()?;
        let result = e.engine.render(request)?;
        if request.cancel.is_cancelled() {
            return Err(EngineError::Cancelled);
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Deterministic PNG fake; delay and failure are scripted without infrastructure.
    struct Fake {
        root: PathBuf,
        delay: std::time::Duration,
        failure: Option<EngineError>,
    }
    impl ImageEngine for Fake {
        fn check(&self) -> Availability {
            Availability::Ready
        }
        fn render(&self, r: &RenderRequest) -> Result<Rendered, EngineError> {
            let until = std::time::Instant::now() + self.delay;
            while std::time::Instant::now() < until {
                if r.cancel.is_cancelled() {
                    return Err(EngineError::Cancelled);
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            if let Some(error) = self.failure {
                return Err(error);
            }
            std::fs::create_dir(&self.root).map_err(|_| EngineError::Io)?;
            let png: &[u8] = &[
                137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0,
                1, 8, 6, 0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 11, 73, 68, 65, 84, 120, 156, 99, 96,
                0, 2, 0, 0, 5, 0, 1, 122, 94, 171, 63, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96,
                130,
            ];
            let path = self.root.join("image.png");
            std::fs::write(&path, png).map_err(|_| EngineError::Io)?;
            Ok(Rendered {
                path,
                staging: self.root.clone(),
                bytes: png.len() as u64,
                width: 1,
                height: 1,
                seed: r.seed,
            })
        }
    }
    struct Failure;
    impl ImageEngine for Failure {
        fn check(&self) -> Availability {
            Availability::Ready
        }
        fn render(&self, _: &RenderRequest) -> Result<Rendered, EngineError> {
            Err(EngineError::Failed)
        }
    }
    #[test]
    fn validation_and_failure_release() {
        let registry = Registry::default();
        registry
            .replace(vec![("fake".into(), "Fake".into(), Arc::new(Failure))])
            .unwrap();
        assert_eq!(registry.list()[0].status, Availability::Unchecked);
        assert_eq!(
            registry.check("fake").unwrap().unwrap().status,
            Availability::Ready
        );
        assert!(registry.check("absent").unwrap().is_none());
        let mut request = RenderRequest {
            prompt: String::new(),
            seed: u32::MAX,
            cancel: Default::default(),
        };
        assert_eq!(
            registry.render("fake", &request).err(),
            Some(EngineError::InvalidRequest)
        );
        request.prompt = "synthetic".into();
        for _ in 0..2 {
            assert_eq!(
                registry.render("fake", &request).err(),
                Some(EngineError::Failed)
            );
        }
        // The same global admission applies across distinct registry instances.
        struct Delayed {
            entered: std::sync::mpsc::Sender<()>,
            release: Mutex<std::sync::mpsc::Receiver<()>>,
        }
        impl ImageEngine for Delayed {
            fn check(&self) -> Availability {
                Availability::Ready
            }
            fn render(&self, _: &RenderRequest) -> Result<Rendered, EngineError> {
                self.entered.send(()).unwrap();
                self.release.lock().unwrap().recv().unwrap();
                Err(EngineError::Failed)
            }
        }
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let active = Arc::new(Registry::default());
        active
            .replace(vec![(
                "slow".into(),
                "Slow".into(),
                Arc::new(Delayed {
                    entered: entered_tx,
                    release: Mutex::new(release_rx),
                }),
            )])
            .unwrap();
        let worker = active.clone();
        let thread = std::thread::spawn(move || {
            worker
                .render(
                    "slow",
                    &RenderRequest {
                        prompt: "synthetic".into(),
                        seed: 0,
                        cancel: Default::default(),
                    },
                )
                .err()
        });
        entered_rx.recv().unwrap();
        assert_eq!(
            registry.render("fake", &request).err(),
            Some(EngineError::Busy)
        );
        assert_eq!(active.replace(vec![]), Err(EngineError::Busy));
        assert_eq!(active.check("slow").err(), Some(EngineError::Busy));
        assert_eq!(active.list().len(), 1); // Listing never waits for inference.
        release_tx.send(()).unwrap();
        assert_eq!(thread.join().unwrap(), Some(EngineError::Failed));
        let run = crate::agent::Run::new();
        run.request_cancel();
        assert_eq!(
            registry
                .render(
                    "fake",
                    &RenderRequest {
                        prompt: "synthetic".into(),
                        seed: 0,
                        cancel: run.token()
                    }
                )
                .err(),
            Some(EngineError::Cancelled)
        );
        let fake = Fake {
            root: std::env::temp_dir().join(format!("diary-fake-image-{}", std::process::id())),
            delay: std::time::Duration::ZERO,
            failure: None,
        };
        let result = fake.render(&request).unwrap();
        assert_eq!((result.width, result.height, result.seed), (1, 1, u32::MAX));
        let path = result.path.clone();
        drop(result);
        assert!(!path.exists());
        struct CancelOnSuccess(Fake);
        impl ImageEngine for CancelOnSuccess {
            fn check(&self) -> Availability {
                Availability::Ready
            }
            fn render(&self, r: &RenderRequest) -> Result<Rendered, EngineError> {
                let result = self.0.render(r)?;
                crate::agent::Run::from(r.cancel.clone()).request_cancel();
                Ok(result)
            }
        }
        let root = fake.root.clone();
        registry
            .replace(vec![(
                "race".into(),
                "Race".into(),
                Arc::new(CancelOnSuccess(fake)),
            )])
            .unwrap();
        let race = RenderRequest {
            prompt: "synthetic".into(),
            seed: 1,
            cancel: Default::default(),
        };
        assert_eq!(
            registry.render("race", &race).err(),
            Some(EngineError::Cancelled)
        );
        assert!(!root.exists());
        for error in [
            EngineError::Timeout,
            EngineError::Cancelled,
            EngineError::InvalidOutput,
        ] {
            registry
                .replace(vec![(
                    "failure".into(),
                    "Failure".into(),
                    Arc::new(Fake {
                        root: root.clone(),
                        delay: std::time::Duration::ZERO,
                        failure: Some(error),
                    }),
                )])
                .unwrap();
            assert_eq!(registry.render("failure", &request).err(), Some(error));
        }
        request.prompt = "x".repeat(32769);
        assert_eq!(request.validate(), Err(EngineError::InvalidRequest));
    }
}
