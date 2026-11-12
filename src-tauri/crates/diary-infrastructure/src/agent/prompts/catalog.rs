use super::renderer::TemplateRenderer;
use super::*;
use sha2::{Digest, Sha256};
use std::{collections::HashMap, sync::Arc};

pub struct EmbeddedPromptCatalog {
    map: HashMap<(AgentKind, &'static str), Result<ParsedPromptAsset, &'static str>>,
}

fn known(name: &str) -> Option<&'static str> {
    [GENERATE, CHAT, SESSION, PARAGRAPH_REVIEW, ILLUSTRATED]
        .into_iter()
        .find(|n| *n == name)
}

impl EmbeddedPromptCatalog {
    pub fn embedded() -> Self {
        Self::from_table(&[EMBEDDED, ILLUSTRATED_ASSETS].concat())
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
    pub(super) fn get(
        &self,
        kind: AgentKind,
        name: &'static str,
    ) -> Result<&ParsedPromptAsset, AssetError> {
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
    pub fn required(kind: AgentKind, action: PromptAction) -> Vec<&'static str> {
        let task = match action {
            PromptAction::Chat => CHAT,
            PromptAction::Illustrated => ILLUSTRATED,
            PromptAction::GenerateCandidate => GENERATE,
        };
        let mut v = vec![task];
        if kind.spec().session_asset {
            v.push(SESSION);
        }
        v
    }
    pub fn state(&self, kind: AgentKind, action: PromptAction) -> Vec<AssetState> {
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
        action: PromptAction,
        adapter_registered: bool,
    ) -> Capability {
        capability(&self.state(kind, action), adapter_registered)
    }
}

impl PromptResolver for EmbeddedPromptCatalog {
    fn resolve(&self, selection: PromptSelection) -> Result<ResolvedPrompt, AgentError> {
        let name = match selection.action {
            PromptAction::Illustrated => ILLUSTRATED,
            PromptAction::Chat => CHAT,
            PromptAction::GenerateCandidate => GENERATE,
        };
        let task = self
            .get(selection.provider, name)
            .map_err(|_| AgentErrorCode::AssetsUnavailable)?;
        let session = if selection.provider.spec().session_asset {
            Some(
                self.get(selection.provider, SESSION)
                    .map_err(|_| AgentErrorCode::AssetsUnavailable)?,
            )
        } else {
            None
        };
        // Length-prefix exact source bytes, including metadata, and the optional paired part.
        let mut hash = Sha256::new();
        for part in [Some(task), session] {
            match part {
                Some(part) => {
                    hash.update([1]);
                    hash.update((part.source.len() as u64).to_be_bytes());
                    hash.update(part.source.as_bytes());
                }
                None => hash.update([0]),
            }
        }
        let revision = hash
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let provenance = PromptProvenance {
            id: PromptId::parse(&format!("{}.{}", selection.provider.asset_dir(), name))?,
            revision: PromptRevision::parse(&revision)?,
        };
        Ok(ResolvedPrompt::new(
            provenance,
            selection,
            task.settings.clone(),
            Arc::new(TemplateRenderer {
                task: task.template.clone(),
                session: session.map(|part| part.template.clone()),
            }),
        ))
    }
}
