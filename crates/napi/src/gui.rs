//! Desktop window host and page IPC.
//!
//! The host runs on the JavaScript thread that created it. `openGui`, `pumpGui`,
//! `closeGui` and `guiStatus` are the only ways to drive the window. Page
//! messages are served by the same core calls as the CLI, and they are audited
//! before a reply is produced.

use std::cell::RefCell;

use base64::Engine as _;
use open_viewer_core::{self as core, AuditMode, CapabilityState, Error as CoreError, Operation, Platform, Result as CoreResult};
use serde_json::{Value, json};
use wae::{
    audit::Mode as WaeMode,
    webview::{self, IpcHandler, IpcOutcome, OpenOptions, Platform as WebViewPlatform, WebViewError, WebViewHost},
};

use crate::audit::{self, Summary};

thread_local! {
    static HOST: RefCell<Option<WebViewHost>> = const { RefCell::new(None) };
}

/// Open the window. Re-opening an already open host is rejected.
pub fn open(options: OpenOptions) -> CoreResult<()> {
    HOST.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_some() {
            return Err(CoreError::invalid("the viewer window is already open"));
        }
        let host = webview::open(options, Box::new(PageHandler)).map_err(webview_error)?;
        *slot = Some(host);
        Ok(())
    })
}

/// Pump pending platform messages. Returns `false` when the window is gone.
pub fn pump() -> bool {
    HOST.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(host) = slot.as_mut()
        else {
            return false;
        };
        let alive = host.pump();
        if !alive {
            *slot = None;
        }
        alive
    })
}

/// Request window close.
pub fn close() -> CoreResult<()> {
    let host = HOST.with(|slot| slot.borrow_mut().take());
    match host {
        Some(mut host) => host.close().map_err(webview_error),
        None => Ok(()),
    }
}

/// Whether a live window exists.
pub fn status() -> bool {
    HOST.with(|slot| slot.borrow().as_ref().is_some_and(WebViewHost::is_alive))
}

struct PageHandler;

impl IpcHandler for PageHandler {
    fn on_ipc(&mut self, message: &str) -> IpcOutcome {
        handle(message)
    }
}

fn handle(message: &str) -> IpcOutcome {
    let request: Value = match serde_json::from_str(message) {
        Ok(value) => value,
        Err(error) => return reply_error(Value::Null, "invalid_argument", &error.to_string()),
    };
    let request_id = request.get("requestId").cloned().unwrap_or(Value::Null);

    match request.get("type").and_then(Value::as_str).unwrap_or_default() {
        "preview" => {
            let Some(path) = request.get("path").and_then(Value::as_str)
            else {
                return reply_error(request_id, "invalid_argument", "preview requires a `path` string");
            };
            let max_edge = request.get("maxEdge").and_then(Value::as_u64).unwrap_or(1024) as u32;
            let params = json!({ "path": path, "maxEdge": max_edge });
            match audit::run(Operation::Preview.capability(), mode(Operation::Preview), params, || {
                let info = core::info(path)?;
                let thumb = core::preview(path, max_edge)?;
                let encoded = base64::engine::general_purpose::STANDARD.encode(&thumb.png);
                let summary = format!("{}x{} -> {}x{}", thumb.source_width, thumb.source_height, thumb.width, thumb.height);
                let preview = json!({
                    "info": info,
                    "sourceWidth": thumb.source_width,
                    "sourceHeight": thumb.source_height,
                    "width": thumb.width,
                    "height": thumb.height,
                    "pngBase64": encoded,
                    "dataUrl": format!("data:image/png;base64,{encoded}"),
                });
                Ok((preview, Summary::new(1, summary)))
            }) {
                Ok(preview) => reply(json!({ "type": "previewResult", "requestId": request_id, "preview": preview })),
                Err(error) => reply_error(request_id, error.code(), &error.to_string()),
            }
        }
        "capabilities" => {
            match audit::run(Operation::Capabilities.capability(), mode(Operation::Capabilities), json!({}), || {
                let report = core::capability_report();
                let count = report.entries.len() as u64;
                let summary = format!("{} capability entries", report.entries.len());
                Ok((report, Summary::new(count, summary)))
            }) {
                Ok(report) => reply(json!({ "type": "capabilitiesResult", "requestId": request_id, "report": report })),
                Err(error) => reply_error(request_id, error.code(), &error.to_string()),
            }
        }
        "close" => {
            let _ = audit::run(Operation::GuiClose.capability(), mode(Operation::GuiClose), json!({}), || {
                Ok(((), Summary::new(0, "window close requested")))
            });
            IpcOutcome::CloseWindow
        }
        _ => IpcOutcome::Ignored,
    }
}

fn reply(value: Value) -> IpcOutcome {
    IpcOutcome::ReplyToPage(value.to_string())
}

fn reply_error(request_id: Value, code: &str, message: &str) -> IpcOutcome {
    reply(json!({ "type": "error", "requestId": request_id, "code": code, "message": message }))
}

fn mode(operation: Operation) -> WaeMode {
    match operation.mode() {
        AuditMode::DryRun => WaeMode::DryRun,
        AuditMode::Execute => WaeMode::Execute,
    }
}

fn webview_error(error: WebViewError) -> CoreError {
    match (error.capability(), error.platform()) {
        (Some(capability), Some(platform)) => {
            let platform = match platform {
                WebViewPlatform::Windows => Platform::Windows,
                WebViewPlatform::Macos => Platform::Macos,
                WebViewPlatform::Linux => Platform::Linux,
            };
            CoreError::capability(
                CapabilityState::unsupported_platform(platform),
                format!("webview capability {capability:?} is unavailable: {error}"),
            )
        }
        _ => CoreError::message(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use wae::webview::Capability;

    use super::*;

    #[test]
    fn preview_without_a_path_is_an_explicit_error() {
        let outcome = handle(r#"{"type":"preview","requestId":7}"#);
        match outcome {
            IpcOutcome::ReplyToPage(text) => {
                let value: Value = serde_json::from_str(&text).unwrap();
                assert_eq!(value["type"], "error");
                assert_eq!(value["code"], "invalid_argument");
                assert_eq!(value["requestId"], 7);
            }
            _ => panic!("expected a reply"),
        }
    }

    #[test]
    fn unknown_messages_are_ignored() {
        assert!(matches!(handle("not json"), IpcOutcome::ReplyToPage(_)));
        assert!(matches!(handle(r#"{"type":"unknown"}"#), IpcOutcome::Ignored));
    }

    #[test]
    fn close_message_closes_the_window() {
        assert!(matches!(handle(r#"{"type":"close"}"#), IpcOutcome::CloseWindow));
    }

    #[test]
    fn capability_mapping_keeps_structured_refusals() {
        let error = webview_error(WebViewError::Unsupported { capability: Capability::Window, platform: WebViewPlatform::Linux });
        assert_eq!(error.code(), "unsupported_platform");
    }
}
