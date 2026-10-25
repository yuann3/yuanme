//! The dev server's HTTP side (#11, #68): every file of the snapshot,
//! with the dev client on each HTML file, a WebSocket that tells the dev
//! client what to reload and show, and the last build's status.

use std::sync::{Arc, Mutex, RwLock};

use axum::Router;
use axum::body::Body as HttpBody;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Request, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde_json::{Value, json};
use tokio::sync::broadcast;
use yuanme_diag::{Diagnostic, Files, Level};
use yuanme_engine::{Body, Build, Cache, image};

use super::Snapshot;
use crate::events::{Console, Event};
use crate::{ENGINE_VERSION, Out};

/// Where the dev client connects. Messages to it are `{"type": "reload",
/// "urls": [...]}`, the `reload` event's, and `{"type": "overlay",
/// "failed": bool, "diagnostics": [...]}`, the rendered errors to show,
/// none to hide it; it sends `browser-console` events' shape back.
pub const SOCKET: &str = "/__yuanme/ws";
/// The last build, as [`Status::json`].
pub const STATUS: &str = "/__yuanme/status";

pub struct Shared {
    pub snapshot: RwLock<Arc<Snapshot>>,
    pub cache: Arc<Mutex<Cache>>,
    /// Each message to dev clients, as JSON.
    pub messages: broadcast::Sender<String>,
    pub status: RwLock<Status>,
    pub out: Out,
}

/// The last build, and once they are checked, the type errors of the
/// last good one (#60).
pub struct Status {
    /// `build-finished`'s fields, and its Diagnostics as `--json` prints
    /// them.
    pub json: Value,
    /// The overlay message.
    pub overlay: String,
    /// Which build of the server's it is, from 0.
    pub build: usize,
}

impl Status {
    /// `diagnostics` are the build's, rendered from `files`, and the type
    /// errors when they are checked.
    pub fn new(
        build: &Build,
        diagnostics: &[Diagnostic],
        files: &Files,
        duration_ms: u128,
        changed_urls: Vec<String>,
    ) -> Status {
        let count = |level| diagnostics.iter().filter(|d| d.level == level).count();
        let errors = count(Level::Error);
        let pages = match (&build.site, build.failed()) {
            (Some(site), false) => site.routes.len(),
            _ => 0,
        };
        let shown: Vec<String> = diagnostics
            .iter()
            .filter(|d| d.level == Level::Error)
            .map(|d| d.render(files, false))
            .collect();
        let overlay = json!({ "type": "overlay", "failed": build.failed(), "diagnostics": shown });
        let diagnostics: Vec<Value> = diagnostics.iter().map(|d| d.to_json(files)).collect();
        Status {
            json: json!({
                "ok": errors == 0,
                "duration_ms": duration_ms,
                "pages": pages,
                "changed_urls": changed_urls,
                "errors": errors,
                "warnings": count(Level::Warning),
                "diagnostics": diagnostics,
            }),
            overlay: overlay.to_string(),
            build: 0,
        }
    }

    /// Adds the type errors of the build that succeeded, which the overlay
    /// then shows.
    pub fn add_type_errors(&mut self, errors: &[Diagnostic], files: &Files) {
        let json = &mut self.json;
        let all = json["diagnostics"].as_array_mut().expect("a list");
        all.extend(errors.iter().map(|d| d.to_json(files)));
        json["errors"] = json!(json["errors"].as_u64().expect("a count") + errors.len() as u64);
        json["ok"] = json!(false);
        let shown: Vec<String> = errors.iter().map(|d| d.render(files, false)).collect();
        let overlay = json!({ "type": "overlay", "failed": false, "diagnostics": shown });
        self.overlay = overlay.to_string();
    }

    pub fn shows_errors(&self) -> bool {
        self.json["errors"] != 0
    }
}

pub fn router(shared: Arc<Shared>) -> Router {
    Router::new()
        .route(SOCKET, get(socket))
        .route(STATUS, get(status))
        .fallback(file)
        .with_state(shared)
}

async fn status(State(shared): State<Arc<Shared>>) -> Response {
    let json = shared
        .status
        .read()
        .expect("never poisoned")
        .json
        .to_string();
    (
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        json,
    )
        .into_response()
}

async fn file(State(shared): State<Arc<Shared>>, request: Request) -> Response {
    let snapshot = shared
        .snapshot
        .read()
        .expect("a swap doesn't panic")
        .clone();
    let Some(path) = decode(request.uri().path()) else {
        return (StatusCode::BAD_REQUEST, "the path isn't UTF-8").into_response();
    };
    let path = path.trim_start_matches('/');
    let files = &snapshot.build.files;
    let index = format!("{path}index.html");
    let found = match path.is_empty() || path.ends_with('/') {
        true => files.get_key_value(index.as_str()),
        false => files.get_key_value(path),
    };
    if let Some((path, output)) = found {
        return send(&snapshot, path, &output.body, StatusCode::OK);
    }
    // The first build failed, so there is no Site to serve yet.
    if snapshot.build.failed() {
        let overlay = shared
            .status
            .read()
            .expect("never poisoned")
            .overlay
            .clone();
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            [
                (header::CONTENT_TYPE, "text/html; charset=utf-8"),
                (header::CACHE_CONTROL, "no-store"),
            ],
            failed_page(&overlay),
        )
            .into_response();
    }
    if let Some((variant, settings)) = snapshot.build.images.get(path) {
        let (variant, settings) = (variant.clone(), settings.clone());
        let path = path.to_string();
        let encoded = tokio::task::spawn_blocking(move || {
            match encode(&shared.cache, &snapshot, &variant, &settings) {
                Ok(body) => send(&snapshot, &path, &body, StatusCode::OK),
                Err(message) => (StatusCode::INTERNAL_SERVER_ERROR, message).into_response(),
            }
        });
        return encoded.await.expect("encoding doesn't panic");
    }
    // A Route's folder without its slash, as static hosts redirect it.
    if files.contains_key(&format!("{path}/index.html")) {
        let query = request
            .uri()
            .query()
            .map_or(String::new(), |q| format!("?{q}"));
        let to = format!("{}/{query}", request.uri().path());
        return (StatusCode::PERMANENT_REDIRECT, [(header::LOCATION, to)]).into_response();
    }
    match files.get_key_value("404.html") {
        Some((path, output)) => send(&snapshot, path, &output.body, StatusCode::NOT_FOUND),
        None => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

/// What a browser is served while no build has succeeded: the errors,
/// and a script that loads the Page again once one does.
fn failed_page(overlay: &str) -> String {
    let errors: Value = serde_json::from_str(overlay).expect("JSON");
    let mut text = String::new();
    for d in errors["diagnostics"].as_array().into_iter().flatten() {
        text.push_str(d.as_str().unwrap_or_default());
        text.push_str("\n\n");
    }
    let text = text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    format!(
        "<!doctype html>\n<html lang=\"en\"><meta charset=\"utf-8\"><title>The build failed</title>\n<h1>The build failed</h1>\n<p>yuanme serves the Site once a save fixes these errors.</p>\n<pre>{text}</pre>\n<script>new WebSocket(`ws://${{location.host}}{SOCKET}`).onmessage = (e) => JSON.parse(e.data).type == \"reload\" && location.reload();</script>\n"
    )
}

/// The file at `path` of the snapshot, never cached by the browser: the
/// next save may change it. An HTML file gets the dev client.
fn send(snapshot: &Snapshot, path: &str, body: &Body, status: StatusCode) -> Response {
    let bytes = match body {
        Body::Bytes(bytes) => Ok(bytes.to_vec()),
        Body::Input(input) => {
            let root = snapshot.inputs.root().expect("dev loads a project folder");
            std::fs::read(root.join(input))
        }
        Body::Cached(file) => std::fs::read(file),
    };
    let bytes = match (&snapshot.build.client, bytes) {
        (Some(client), Ok(bytes)) if path.ends_with(".html") => Ok(with_client(bytes, client)),
        (_, bytes) => bytes,
    };
    match bytes {
        Ok(bytes) => (
            status,
            [
                (header::CONTENT_TYPE, content_type(path)),
                (header::CACHE_CONTROL, "no-store"),
            ],
            HttpBody::from(bytes),
        )
            .into_response(),
        Err(e) => {
            let message = format!("can't read {path}: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, message).into_response()
        }
    }
}

/// `html` with a `<script>` of the dev client at `src` before its
/// `</head>`, or else before its `<body>` or at its end.
fn with_client(mut html: Vec<u8>, src: &str) -> Vec<u8> {
    let tag = format!("<script type=\"module\" src=\"/{src}\" data-yuanme-client></script>");
    let find = |needle: &[u8]| {
        html.windows(needle.len())
            .position(|w| w.eq_ignore_ascii_case(needle))
    };
    let at = find(b"</head")
        .or_else(|| find(b"<body"))
        .unwrap_or(html.len());
    html.splice(at..at, tag.into_bytes());
    html
}

/// An image variant the snapshot's Pages name, from the cache or encoded
/// now (#24). The encoder runs without the cache's lock, so a save never
/// waits for a photo.
fn encode(
    cache: &Mutex<Cache>,
    snapshot: &Snapshot,
    variant: &image::Variant,
    settings: &image::Settings,
) -> Result<Body, String> {
    if let Some(body) = cache
        .lock()
        .expect("a build doesn't panic")
        .get_large(&variant.key)
    {
        return Ok(body);
    }
    let mut scratch = Cache::memory(ENGINE_VERSION);
    let variants = std::slice::from_ref(variant);
    let encoded = image::encode(&mut scratch, &snapshot.inputs, variants, settings);
    let output = encoded
        .into_iter()
        .next()
        .expect("one per variant")
        .map_err(|e| format!("{}: {e}", variant.source))?;
    let Body::Bytes(bytes) = &output.body else {
        unreachable!("a memory cache holds its bytes")
    };
    let mut cache = cache.lock().expect("a build doesn't panic");
    cache
        .insert_large(variant.key, bytes)
        .map_err(|e| format!("can't write the cache: {e}"))?;
    Ok(output.body)
}

async fn socket(State(shared): State<Arc<Shared>>, upgrade: WebSocketUpgrade) -> Response {
    upgrade.on_upgrade(move |socket| client(shared, socket))
}

/// One dev client: reloads and the overlay go out, console output comes
/// in.
async fn client(shared: Arc<Shared>, mut socket: WebSocket) {
    let mut messages = shared.messages.subscribe();
    let shown = {
        let status = shared.status.read().expect("never poisoned");
        status.shows_errors().then(|| status.overlay.clone())
    };
    if let Some(overlay) = shown
        && socket.send(Message::Text(overlay.into())).await.is_err()
    {
        return;
    }
    loop {
        enum Next {
            In(Option<Result<Message, axum::Error>>),
            Out(Result<String, broadcast::error::RecvError>),
        }
        let next = tokio::select! {
            message = socket.recv() => Next::In(message),
            message = messages.recv() => Next::Out(message),
        };
        match next {
            Next::In(Some(Ok(Message::Text(text)))) => {
                if let Some(console) = Console::parse(&text) {
                    console_line(shared.out, console);
                }
            }
            Next::In(Some(Ok(_))) => {}
            Next::In(_) => return,
            Next::Out(Ok(json)) => {
                if socket.send(Message::Text(json.into())).await.is_err() {
                    return;
                }
            }
            // A client that fell behind missed reloads: closing tells it
            // to load the Page again whole.
            Next::Out(Err(_)) => return,
        }
    }
}

/// A browser's console output, in the terminal (#21).
fn console_line(out: Out, console: Console) {
    if !out.json && !out.quiet {
        let mut line = format!(
            "browser {}: {} ({})",
            console.level, console.message, console.url
        );
        if let Some(stack) = &console.stack {
            line.push('\n');
            line.push_str(stack);
        }
        eprintln!("{line}");
    }
    out.event(Event::BrowserConsole(console));
}

/// The path with its `%XX` escapes decoded, or `None` if that isn't UTF-8.
fn decode(path: &str) -> Option<String> {
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(byte)) => {
                out.push(byte);
                i += 3;
            }
            (byte, _) => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

fn content_type(path: &str) -> &'static str {
    let ext = path.rsplit_once('.').map_or("", |(_, e)| e);
    match ext.to_ascii_lowercase().as_str() {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" | "map" => "application/json",
        "xml" => "application/xml",
        "txt" => "text/plain; charset=utf-8",
        "svg" => "image/svg+xml",
        "avif" => "image/avif",
        "webp" => "image/webp",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "otf" => "font/otf",
        "ttf" => "font/ttf",
        "pdf" => "application/pdf",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}
