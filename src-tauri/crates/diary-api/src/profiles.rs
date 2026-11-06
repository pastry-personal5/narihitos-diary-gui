//! Read-only agent profile status. Listing reports the last cached check; only the explicit check
//! route spawns a probe, and neither starts inference. Responses carry no path, stderr or content.
use super::{ServerState, failure, imports};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use diary_application::agent::{Capability, CapabilityReason, ProfileStatus, ProfileView};
use serde_json::{Value, json};
use std::sync::Arc;

fn status(s: Option<ProfileStatus>) -> &'static str {
    match s {
        None => "unchecked",
        Some(ProfileStatus::Missing) => "missing",
        Some(ProfileStatus::UnsupportedVersion) => "unsupported_version",
        Some(ProfileStatus::Unavailable) => "unavailable",
        Some(ProfileStatus::AvailableAuthUnknown) => "available_auth_unknown",
        Some(ProfileStatus::Unauthenticated) => "unauthenticated",
        Some(ProfileStatus::Ready) => "ready",
        Some(ProfileStatus::Busy) => "busy",
        Some(ProfileStatus::Failed) => "failed",
    }
}
fn capability(c: Capability) -> Value {
    match c {
        Capability::Ready => json!({"available":true,"reason":null}),
        Capability::Unavailable(r) => json!({"available":false,"reason":match r {
            CapabilityReason::AssetsMissing => "assets_missing",
            CapabilityReason::AssetsInvalid => "assets_invalid",
            CapabilityReason::AdapterUnimplemented => "adapter_unimplemented",
        }}),
    }
}
fn view(v: &ProfileView) -> Value {
    json!({"alias":v.alias,"type":v.kind.asset_dir(),"status":status(v.status),
        "capabilities":{"chat":capability(v.chat),"generate":capability(v.generate)}})
}

pub(super) async fn list(State(s): State<Arc<ServerState>>, headers: HeaderMap) -> Response {
    if let Err(r) = imports::authorize(&s, &headers) {
        return r;
    }
    let Some(dir) = s.profiles.clone() else {
        return Json(json!({"profiles":[]})).into_response();
    };
    match tokio::task::spawn_blocking(move || dir.list()).await {
        Ok(list) => {
            Json(json!({"profiles":list.iter().map(view).collect::<Vec<_>>()})).into_response()
        }
        Err(_) => failure(StatusCode::INTERNAL_SERVER_ERROR, "internal"),
    }
}

pub(super) async fn check(
    State(s): State<Arc<ServerState>>,
    Path(alias): Path<String>,
    headers: HeaderMap,
) -> Response {
    if let Err(r) = imports::authorize(&s, &headers) {
        return r;
    }
    let Some(dir) = s.profiles.clone() else {
        return failure(StatusCode::NOT_FOUND, "not_found");
    };
    match tokio::task::spawn_blocking(move || dir.check(&alias)).await {
        Ok(Some(v)) => Json(view(&v)).into_response(),
        Ok(None) => failure(StatusCode::NOT_FOUND, "not_found"),
        Err(_) => failure(StatusCode::INTERNAL_SERVER_ERROR, "internal"),
    }
}

#[cfg(test)]
mod tests;
