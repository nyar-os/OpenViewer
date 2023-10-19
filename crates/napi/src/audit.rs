//! Audit trail at the N-API capability boundary.
//!
//! Every exported capability is recorded through [`run`], including the GUI IPC
//! path, so the CLI and the window cannot bypass attribution. The schema,
//! redaction, rotation and query implementation come from the shared
//! `wae.audit/v0` contract. This module only supplies OpenViewer's domain fields.

use std::sync::{Arc, PoisonError, RwLock};

use open_viewer_core::{Error as CoreError, Result as CoreResult};
use serde_json::Value;
use wae::audit::{Actor, Auditor, Entry, Mode, Outcome, Query};

const APP: &str = "open-viewer";

static AUDITOR: RwLock<Option<Arc<Auditor>>> = RwLock::new(None);

/// Target count and human-readable summary attached to a record.
#[derive(Debug, Clone)]
pub struct Summary {
    /// Number of targets the call touched.
    pub count: u64,
    /// Human-readable summary.
    pub summary: String,
}

impl Summary {
    /// Build a summary.
    pub fn new(count: u64, summary: impl Into<String>) -> Self {
        Self { count, summary: summary.into() }
    }
}

fn auditor() -> Arc<Auditor> {
    if let Some(existing) = AUDITOR.read().unwrap_or_else(PoisonError::into_inner).as_ref() {
        return Arc::clone(existing);
    }
    let built = Arc::new(Auditor::from_env(APP, env!("CARGO_PKG_VERSION")));
    let mut guard = AUDITOR.write().unwrap_or_else(PoisonError::into_inner);
    Arc::clone(guard.get_or_insert(built))
}

/// Replace the process-wide auditor after an explicit actor or session override.
pub fn configure(actor: Actor, session: Option<String>, host: Option<String>) {
    let mut builder = Auditor::builder(APP, env!("CARGO_PKG_VERSION")).actor(actor);
    if let Some(session) = session {
        builder = builder.session(session);
    }
    if let Some(host) = host {
        builder = builder.host(host);
    }
    let configured = Arc::new(builder.build());
    let actor = configured.actor();
    let params = serde_json::json!({
        "actorKind": actor.kind,
        "actorId": actor.id,
        "actorSource": actor.source,
        "session": configured.session(),
        "host": configured.host(),
    });
    let _ = run_with(&configured, "audit-configure", Mode::DryRun, params, || Ok(((), Summary::new(0, "audit configuration updated"))));
    *AUDITOR.write().unwrap_or_else(PoisonError::into_inner) = Some(configured);
}

/// Record one capability call and return its result unchanged.
///
/// Read-only capabilities use [`Mode::DryRun`], capabilities that mutate host
/// state use [`Mode::Execute`]. Capability refusals are stored as `rejected`,
/// all other failures as `failure`.
pub fn run<T>(capability: &str, mode: Mode, params: Value, body: impl FnOnce() -> CoreResult<(T, Summary)>) -> CoreResult<T> {
    let auditor = auditor();
    run_with(&auditor, capability, mode, params, body)
}

fn run_with<T>(
    auditor: &Auditor,
    capability: &str,
    mode: Mode,
    params: Value,
    body: impl FnOnce() -> CoreResult<(T, Summary)>,
) -> CoreResult<T> {
    let call = auditor.call(capability, mode).params(params);
    match body() {
        Ok((value, summary)) => {
            let _ = call.targets(summary.count, summary.summary).succeed();
            Ok(value)
        }
        Err(error) => {
            let code = error.code().to_string();
            match error_outcome(&error) {
                Outcome::Rejected => {
                    let _ = call.reject(code);
                }
                _ => {
                    let _ = call.fail(code);
                }
            }
            Err(error)
        }
    }
}

/// Query audit entries with the shared filter shape.
pub fn query(filter: &Query) -> Result<Vec<Entry>, wae::audit::AuditError> {
    auditor().query(filter)
}

fn error_outcome(error: &CoreError) -> Outcome {
    if error.capability_state().is_some() { Outcome::Rejected } else { Outcome::Failure }
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use open_viewer_core::CapabilityState;
    use wae::audit::ActorKind;

    use super::*;

    fn test_auditor() -> (Auditor, PathBuf) {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!("open-viewer-audit-{}-{}", std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed)));
        let actor = Actor { kind: ActorKind::Agent, id: "agent-1".into(), source: "test".into() };
        let auditor = Auditor::builder(APP, "0.0.0").actor(actor).session("session-1").host("host-1").dir(&dir).build();
        (auditor, dir)
    }

    #[test]
    fn success_is_recorded_with_targets() {
        let (auditor, dir) = test_auditor();
        run_with(&auditor, "image-info", Mode::DryRun, serde_json::json!({ "path": "C:/images/a.png" }), || {
            Ok((42, Summary::new(1, "one image")))
        })
        .unwrap();
        let entries = auditor.query(&Query::default()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].capability, "image-info");
        assert_eq!(entries[0].mode, Mode::DryRun);
        assert_eq!(entries[0].outcome, Outcome::Success);
        assert_eq!(entries[0].targets.count, 1);
        let text = std::fs::read_to_string(dir.join("open-viewer.jsonl")).unwrap();
        assert!(text.contains("\"schema\":\"wae.audit/v0\""));
    }

    #[test]
    fn capability_refusal_is_rejected_instead_of_failed() {
        let (auditor, _) = test_auditor();
        let rejected: CoreResult<()> = run_with(&auditor, "image-heic", Mode::DryRun, Value::Null, || {
            Err(CoreError::capability(CapabilityState::dependency_blocked("missing HEIC decoder"), "HEIC is not available"))
        });
        assert!(rejected.is_err());
        let entries = auditor.query(&Query::default()).unwrap();
        assert_eq!(entries[0].outcome, Outcome::Rejected);
        assert_eq!(entries[0].error_code.as_deref(), Some("dependency_blocked"));
    }

    #[test]
    fn credentials_in_params_are_redacted_before_write() {
        let (auditor, dir) = test_auditor();
        run_with(&auditor, "image-info", Mode::DryRun, serde_json::json!({ "token": "SECRET", "path": "C:/image.png" }), || {
            Ok(((), Summary::new(0, "none")))
        })
        .unwrap();
        let text = std::fs::read_to_string(dir.join("open-viewer.jsonl")).unwrap();
        assert!(!text.contains("SECRET"));
        assert!(text.contains("<redacted>"));
    }
}
