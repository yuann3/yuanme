//! The dev server's HTTP side (#11): every file of the snapshot, and a
//! WebSocket that tells the dev client which Pages to reload.

use std::sync::{Arc, Mutex, RwLock};

use axum::Router;
use axum::body::Body as HttpBody;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Request, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use tokio::sync::broadcast;
use yuanme_engine::{Body, Cache};

use super::Snapshot;
use crate::Out;
use crate::events::{Console, Event};

/// Where the dev client connects. Messages to it are `{"type": "reload",
/// "urls": [...]}`, the `reload` event's; it sends `browser-console`
/// events' shape back.
pub const SOCKET: &str = "/__yuanme/ws";

pub struct Shared {
    pub snapshot: RwLock<Arc<Snapshot>>,
    pub cache: Arc<Mutex<Cache>>,
    /// Each `reload` message, as JSON.
    pub reloads: broadcast::Sender<String>,
    pub out: Out,
}

pub fn router(shared: Arc<Shared>) -> Router {
    Router::new()
        .route(SOCKET, get(socket))
        .fallback(file)
        .with_state(shared)
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

/// The file at `path` of the snapshot, never cached by the browser: the
/// next save may change it.
fn send(snapshot: &Snapshot, path: &str, body: &Body, status: StatusCode) -> Response {
    let bytes = match body {
        Body::Bytes(bytes) => Ok(bytes.to_vec()),
        Body::Input(input) => {
            let root = snapshot.inputs.root().expect("dev loads a project folder");
            std::fs::read(root.join(input))
        }
        Body::Cached(file) => std::fs::read(file),
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

async fn socket(State(shared): State<Arc<Shared>>, upgrade: WebSocketUpgrade) -> Response {
    upgrade.on_upgrade(move |socket| client(shared, socket))
}

/// One dev client: reloads go out, console output comes in.
async fn client(shared: Arc<Shared>, mut socket: WebSocket) {
    let mut reloads = shared.reloads.subscribe();
    loop {
        enum Next {
            In(Option<Result<Message, axum::Error>>),
            Out(Result<String, broadcast::error::RecvError>),
        }
        let next = tokio::select! {
            message = socket.recv() => Next::In(message),
            reload = reloads.recv() => Next::Out(reload),
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
