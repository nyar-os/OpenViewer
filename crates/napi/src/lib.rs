//! Node-API capability surface for OpenViewer.
//!
//! This crate is the only implementation path used by the CLI, the desktop
//! window and agent skills. It moves data between JavaScript and the domain
//! core, records every call in the shared audit trail, and hosts the WebView.
//! It contains no platform `cfg` branches and no platform-specific strings.

#![deny(missing_docs)]

mod audit;
mod gui;

use base64::Engine as _;
use open_viewer_core::{self as core, AuditMode, Error as CoreError, InstallOptions, Operation, UninstallOptions};
use serde_json::json;
use wae::{
    CallContext, Env, Error, JsValue, Result,
    audit::{Actor, ActorKind, Mode as WaeMode},
    function, register_module,
    webview::OpenOptions,
};

use crate::audit::Summary;

/// Library version used by `@wae/open-viewer` to verify the native binary.
#[function]
fn version(ctx: CallContext) -> Result<JsValue> {
    let version = audit::run(Operation::Version.capability(), mode(Operation::Version), json!({}), || {
        Ok((env!("CARGO_PKG_VERSION").to_string(), Summary::new(0, "version")))
    })
    .map_err(core_error)?;
    ctx.env().string(&version)
}

/// Report every capability with the shared three-state vocabulary.
#[function]
fn capabilities(ctx: CallContext) -> Result<JsValue> {
    let report = audit::run(Operation::Capabilities.capability(), mode(Operation::Capabilities), json!({}), || {
        let report = core::capability_report();
        let count = report.entries.len() as u64;
        let summary = format!("{} capability entries on {}", report.entries.len(), report.platform.name());
        Ok((report, Summary::new(count, summary)))
    })
    .map_err(core_error)?;
    json_to_js(ctx.env(), &serde_json::to_value(report).map_err(|error| Error::new(error.to_string()))?)
}

/// Read image metadata.
#[function(js_name = "info")]
fn image_info(ctx: CallContext) -> Result<JsValue> {
    let path = required_path(ctx.args().first().copied(), "info")?;
    let params = json!({ "path": path });
    let info = audit::run(Operation::Info.capability(), mode(Operation::Info), params, || {
        let info = core::info(&path)?;
        let summary = format!("{} {}x{}", info.format, info.width, info.height);
        Ok((info, Summary::new(1, summary)))
    })
    .map_err(core_error)?;
    json_to_js(ctx.env(), &serde_json::to_value(info).map_err(|error| Error::new(error.to_string()))?)
}

/// Decode an image to RGBA8 and return it as base64.
#[function(js_name = "decodeRgba")]
fn decode_rgba(ctx: CallContext) -> Result<JsValue> {
    let path = required_path(ctx.args().first().copied(), "decodeRgba")?;
    let params = json!({ "path": path });
    let payload = audit::run(Operation::Decode.capability(), mode(Operation::Decode), params, || {
        let decoded = core::decode(&path)?;
        let summary = format!("{}x{} RGBA", decoded.width, decoded.height);
        let payload = json!({
            "width": decoded.width,
            "height": decoded.height,
            "bytes": decoded.rgba.len(),
            "rgbaBase64": base64::engine::general_purpose::STANDARD.encode(&decoded.rgba),
        });
        Ok((payload, Summary::new(1, summary)))
    })
    .map_err(core_error)?;
    json_to_js(ctx.env(), &payload)
}

/// Render a preview PNG for the desktop window.
#[function]
fn preview(ctx: CallContext) -> Result<JsValue> {
    let path = required_path(ctx.args().first().copied(), "preview")?;
    let max_edge = optional_u32(ctx.args().first().copied(), "maxEdge")?.unwrap_or(1024);
    let params = json!({ "path": path, "maxEdge": max_edge });
    let payload = audit::run(Operation::Preview.capability(), mode(Operation::Preview), params, || {
        let info = core::info(&path)?;
        let thumb = core::preview(&path, max_edge)?;
        let encoded = base64::engine::general_purpose::STANDARD.encode(&thumb.png);
        let summary = format!("{}x{} -> {}x{}", thumb.source_width, thumb.source_height, thumb.width, thumb.height);
        let payload = json!({
            "info": info,
            "sourceWidth": thumb.source_width,
            "sourceHeight": thumb.source_height,
            "width": thumb.width,
            "height": thumb.height,
            "pngBase64": encoded,
            "dataUrl": format!("data:image/png;base64,{encoded}"),
        });
        Ok((payload, Summary::new(1, summary)))
    })
    .map_err(core_error)?;
    json_to_js(ctx.env(), &payload)
}

/// Render a thumbnail and write it as a PNG file, or preview it with `dryRun`.
#[function(js_name = "writeThumbnail")]
fn write_thumbnail(ctx: CallContext) -> Result<JsValue> {
    let path = required_path(ctx.args().first().copied(), "writeThumbnail")?;
    let out = optional_string(ctx.args().first().copied(), "out")?;
    let max_edge = optional_u32(ctx.args().first().copied(), "maxEdge")?.unwrap_or(1024);
    let dry_run = optional_bool(ctx.args().first().copied(), "dryRun")?.unwrap_or(false);

    let params = json!({ "path": path, "out": out, "maxEdge": max_edge, "dryRun": dry_run });
    let payload = audit::run(Operation::Thumbnail.capability(), mode(Operation::Thumbnail), params, || {
        if dry_run {
            let thumb = core::thumbnail(&path, max_edge)?;
            let summary = format!("{}x{} -> {}x{} (dry run)", thumb.source_width, thumb.source_height, thumb.width, thumb.height);
            let payload = json!({
                "dryRun": true,
                "width": thumb.width,
                "height": thumb.height,
                "sourceWidth": thumb.source_width,
                "sourceHeight": thumb.source_height,
                "bytes": thumb.png.len(),
            });
            return Ok((payload, Summary::new(1, summary)));
        }
        let out = out.ok_or_else(|| CoreError::invalid("writeThumbnail requires an `out` path unless `dryRun` is true"))?;
        let write = core::write_thumbnail(&path, &out, max_edge)?;
        let summary = format!("wrote {} ({} bytes)", write.path, write.bytes);
        let payload = serde_json::to_value(write).map_err(|error| CoreError::message(error.to_string()))?;
        Ok((payload, Summary::new(1, summary)))
    })
    .map_err(core_error)?;
    json_to_js(ctx.env(), &payload)
}

/// Plan an install or upgrade.
#[function(js_name = "installPlan")]
fn install_plan(ctx: CallContext) -> Result<JsValue> {
    let options = install_options(ctx.args().first().copied())?;
    let params = json!({ "noShortcut": options.no_shortcut, "resetConfig": options.reset_config });
    let plan = audit::run(Operation::InstallPlan.capability(), mode(Operation::InstallPlan), params, || {
        let plan = core::plan_install(&options)?;
        let count = plan.actions.len() as u64;
        let summary = format!("{} planned action(s)", count);
        Ok((plan, Summary::new(count, summary)))
    })
    .map_err(core_error)?;
    json_to_js(ctx.env(), &serde_json::to_value(plan).map_err(|error| Error::new(error.to_string()))?)
}

/// Apply an install or upgrade.
#[function(js_name = "installApply")]
fn install_apply(ctx: CallContext) -> Result<JsValue> {
    let options = install_options(ctx.args().first().copied())?;
    let params = json!({ "noShortcut": options.no_shortcut, "resetConfig": options.reset_config });
    audit::run(Operation::InstallApply.capability(), mode(Operation::InstallApply), params, || {
        core::apply_install(&options)?;
        Ok(((), Summary::new(1, "install applied")))
    })
    .map_err(core_error)?;
    ctx.env().undefined()
}

/// Plan an uninstall.
#[function(js_name = "uninstallPlan")]
fn uninstall_plan(ctx: CallContext) -> Result<JsValue> {
    let options = uninstall_options(ctx.args().first().copied())?;
    let params = json!({ "keepConfig": options.keep_config });
    let plan = audit::run(Operation::UninstallPlan.capability(), mode(Operation::UninstallPlan), params, || {
        let plan = core::plan_uninstall(&options)?;
        let count = plan.actions.len() as u64;
        let summary = format!("{} planned action(s)", count);
        Ok((plan, Summary::new(count, summary)))
    })
    .map_err(core_error)?;
    json_to_js(ctx.env(), &serde_json::to_value(plan).map_err(|error| Error::new(error.to_string()))?)
}

/// Apply an uninstall.
#[function(js_name = "uninstallApply")]
fn uninstall_apply(ctx: CallContext) -> Result<JsValue> {
    let options = uninstall_options(ctx.args().first().copied())?;
    let params = json!({ "keepConfig": options.keep_config });
    audit::run(Operation::UninstallApply.capability(), mode(Operation::UninstallApply), params, || {
        core::apply_uninstall(&options)?;
        Ok(((), Summary::new(1, "uninstall applied")))
    })
    .map_err(core_error)?;
    ctx.env().undefined()
}

/// Override actor, session or host before a call.
#[function(js_name = "configureAudit")]
fn configure_audit(ctx: CallContext) -> Result<JsValue> {
    let arg = ctx.args().first().copied();
    let kind = optional_string(arg, "actorKind")?.unwrap_or_else(|| "user".to_string());
    let actor_kind = if kind.eq_ignore_ascii_case("agent") { ActorKind::Agent } else { ActorKind::User };
    let actor_id = optional_string(arg, "actorId")?
        .unwrap_or_else(|| if actor_kind == ActorKind::Agent { "unknown".to_string() } else { "local".to_string() });
    let source = optional_string(arg, "actorSource")?.unwrap_or_else(|| "local".to_string());
    let session = optional_string(arg, "session")?;
    let host = optional_string(arg, "host")?;

    audit::configure(Actor { kind: actor_kind, id: actor_id, source }, session, host);
    audit::run("audit-configure", WaeMode::DryRun, json!({ "actorKind": kind }), || Ok(((), Summary::new(0, "audit configuration updated"))))
        .map_err(core_error)?;
    ctx.env().undefined()
}

/// Query the audit trail. Filter fields: `kind`, `id`, `fromMs`, `toMs`, `capability`, `limit`.
#[function(js_name = "auditQuery")]
fn audit_query(ctx: CallContext) -> Result<JsValue> {
    let arg = ctx.args().first().copied();
    let query = wae::audit::Query {
        actor_kind: match optional_string(arg, "kind")? {
            Some(value) if value.eq_ignore_ascii_case("agent") => Some(ActorKind::Agent),
            Some(_) => Some(ActorKind::User),
            None => None,
        },
        actor_id: optional_string(arg, "id")?,
        from_ms: optional_u64(arg, "fromMs")?,
        to_ms: optional_u64(arg, "toMs")?,
        capability: optional_string(arg, "capability")?,
        limit: optional_u64(arg, "limit")?.map(|value| value as usize),
    };

    let entries = audit::run("audit-query", WaeMode::DryRun, json!({}), || {
        let entries = audit::query(&query).map_err(|error| CoreError::message(error.to_string()))?;
        let count = entries.len() as u64;
        let summary = format!("{} audit record(s)", entries.len());
        Ok((entries, Summary::new(count, summary)))
    })
    .map_err(core_error)?;
    json_to_js(ctx.env(), &serde_json::to_value(entries).map_err(|error| Error::new(error.to_string()))?)
}

/// Open the desktop viewer window.
#[function(js_name = "openGui")]
fn open_gui(ctx: CallContext) -> Result<JsValue> {
    let arg = ctx.args().first().copied();
    let content_dir = optional_string(arg, "contentDir")?;
    let url = optional_string(arg, "url")?;
    if content_dir.is_none() && url.is_none() {
        return Err(Error::new("invalid_argument: openGui requires `contentDir` or `url`"));
    }
    let title = optional_string(arg, "title")?.unwrap_or_else(|| "OpenViewer".to_string());
    let width = optional_u32(arg, "width")?.unwrap_or(1280) as i32;
    let height = optional_u32(arg, "height")?.unwrap_or(860) as i32;
    let params = json!({ "contentDir": content_dir, "url": url, "title": title, "width": width, "height": height });
    audit::run(Operation::GuiOpen.capability(), mode(Operation::GuiOpen), params, || {
        let options = OpenOptions { title, width, height, content_dir, url };
        gui::open(options)?;
        Ok(((), Summary::new(1, "viewer window opened")))
    })
    .map_err(core_error)?;
    ctx.env().undefined()
}

/// Pump platform messages. Returns `false` once the window is gone.
#[function(js_name = "pumpGui")]
fn pump_gui(ctx: CallContext) -> Result<JsValue> {
    let alive = audit::run(Operation::GuiPump.capability(), mode(Operation::GuiPump), json!({}), || {
        let alive = gui::pump();
        Ok((alive, Summary::new(0, if alive { "window alive" } else { "window closed" })))
    })
    .map_err(core_error)?;
    ctx.env().boolean(alive)
}

/// Request window close.
#[function(js_name = "closeGui")]
fn close_gui(ctx: CallContext) -> Result<JsValue> {
    audit::run(Operation::GuiClose.capability(), mode(Operation::GuiClose), json!({}), || {
        gui::close()?;
        Ok(((), Summary::new(0, "window close requested")))
    })
    .map_err(core_error)?;
    ctx.env().undefined()
}

/// Report whether the window is alive.
#[function(js_name = "guiStatus")]
fn gui_status(ctx: CallContext) -> Result<JsValue> {
    let alive = audit::run(Operation::GuiStatus.capability(), mode(Operation::GuiStatus), json!({}), || {
        Ok((gui::status(), Summary::new(0, "window status")))
    })
    .map_err(core_error)?;
    ctx.env().boolean(alive)
}

fn mode(operation: Operation) -> WaeMode {
    match operation.mode() {
        AuditMode::DryRun => WaeMode::DryRun,
        AuditMode::Execute => WaeMode::Execute,
    }
}

fn core_error(error: CoreError) -> Error {
    Error::new(format!("{}: {}", error.code(), error))
}

fn install_options(arg: Option<JsValue>) -> Result<InstallOptions> {
    Ok(InstallOptions {
        no_shortcut: optional_bool(arg, "noShortcut")?.unwrap_or(false),
        reset_config: optional_bool(arg, "resetConfig")?.unwrap_or(false),
    })
}

fn uninstall_options(arg: Option<JsValue>) -> Result<UninstallOptions> {
    Ok(UninstallOptions { keep_config: optional_bool(arg, "keepConfig")?.unwrap_or(false) })
}

fn required_path(arg: Option<JsValue>, function: &str) -> Result<String> {
    let arg = arg.ok_or_else(|| Error::new(format!("{function} requires an options object")))?;
    if arg.is_undefined()? {
        return Err(Error::new(format!("{function} requires an options object")));
    }
    let value = arg.get_named("path")?;
    if value.is_undefined()? {
        return Err(Error::new(format!("{function} requires a `path` string")));
    }
    value.as_string()
}

fn optional_string(arg: Option<JsValue>, name: &str) -> Result<Option<String>> {
    let Some(arg) = arg
    else {
        return Ok(None);
    };
    if arg.is_undefined()? {
        return Ok(None);
    }
    let value = arg.get_named(name)?;
    if value.is_undefined()? {
        return Ok(None);
    }
    Ok(Some(value.as_string()?))
}

fn optional_bool(arg: Option<JsValue>, name: &str) -> Result<Option<bool>> {
    let Some(arg) = arg
    else {
        return Ok(None);
    };
    if arg.is_undefined()? {
        return Ok(None);
    }
    let value = arg.get_named(name)?;
    if value.is_undefined()? {
        return Ok(None);
    }
    Ok(Some(value.as_bool()?))
}

fn optional_u32(arg: Option<JsValue>, name: &str) -> Result<Option<u32>> {
    Ok(optional_u64(arg, name)?.map(|value| u32::try_from(value).unwrap_or(u32::MAX)))
}

fn optional_u64(arg: Option<JsValue>, name: &str) -> Result<Option<u64>> {
    let Some(arg) = arg
    else {
        return Ok(None);
    };
    if arg.is_undefined()? {
        return Ok(None);
    }
    let value = arg.get_named(name)?;
    if value.is_undefined()? {
        return Ok(None);
    }
    Ok(Some(value.as_i64()?.max(0) as u64))
}

fn json_to_js(env: Env, value: &serde_json::Value) -> Result<JsValue> {
    match value {
        serde_json::Value::Null => env.null(),
        serde_json::Value::Bool(inner) => env.boolean(*inner),
        serde_json::Value::Number(number) => {
            if let Some(integer) = number.as_i64() {
                env.int64(integer)
            }
            else {
                env.double(number.as_f64().unwrap_or(0.0))
            }
        }
        serde_json::Value::String(text) => env.string(text),
        serde_json::Value::Array(items) => {
            let array = env.array_with_length(items.len())?;
            for (index, item) in items.iter().enumerate() {
                array.set_element(index as u32, json_to_js(env, item)?)?;
            }
            Ok(array)
        }
        serde_json::Value::Object(map) => {
            let object = env.object()?;
            for (key, item) in map {
                object.set_named(key, json_to_js(env, item)?)?;
            }
            Ok(object)
        }
    }
}

register_module![
    version,
    capabilities,
    image_info,
    decode_rgba,
    preview,
    write_thumbnail,
    install_plan,
    install_apply,
    uninstall_plan,
    uninstall_apply,
    configure_audit,
    audit_query,
    open_gui,
    pump_gui,
    close_gui,
    gui_status
];
