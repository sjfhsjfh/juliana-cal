use std::net::SocketAddr;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};
use axum::{
    Router,
    extract::{Path as AxumPath, State},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use juliana_cal_core::{Manifest, render};
use tracing_subscriber::EnvFilter;

const CONTENT_TYPE: &str = "text/calendar; charset=utf-8";
const CACHE_CONTROL: &str = "public, max-age=300";

pub fn run(root: PathBuf, bind: SocketAddr) -> Result<()> {
    init_tracing();

    let root = root
        .canonicalize()
        .with_context(|| format!("opening {}", root.display()))?;
    if !root.is_dir() {
        anyhow::bail!("{} is not a directory", root.display());
    }

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("creating tokio runtime")?
        .block_on(serve(root, bind))
}

async fn serve(root: PathBuf, bind: SocketAddr) -> Result<()> {
    let app = router(root.clone());
    let listener = tokio::net::TcpListener::bind(bind)
        .await
        .with_context(|| format!("binding {bind}"))?;

    tracing::info!(address = %bind, root = %root.display(), "listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("serving HTTP")
}

fn router(root: PathBuf) -> Router {
    Router::new()
        .route("/{*path}", get(handler))
        .with_state(root)
}

async fn handler(State(root): State<PathBuf>, AxumPath(path): AxumPath<String>) -> Response {
    match respond(&root, &path).await {
        Ok(response) => response,
        Err(error) => {
            tracing::error!(path = %path, error = %error, "request failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to render calendar\n",
            )
                .into_response()
        }
    }
}

async fn respond(root: &Path, request: &str) -> Result<Response> {
    let Some(path) = safe_join(root, request) else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    if path.extension().and_then(|extension| extension.to_str()) != Some("ics") {
        return Ok(StatusCode::NOT_FOUND.into_response());
    }

    if path.is_file() {
        let bytes = tokio::fs::read(&path)
            .await
            .with_context(|| format!("reading {}", path.display()))?;
        return Ok(calendar(bytes));
    }

    let fallback = path.with_extension("toml");
    if fallback.is_file() {
        let source = tokio::fs::read_to_string(&fallback)
            .await
            .with_context(|| format!("reading {}", fallback.display()))?;
        let manifest =
            Manifest::parse(&source).with_context(|| format!("parsing {}", fallback.display()))?;
        let ics = render(&manifest).with_context(|| format!("rendering {}", fallback.display()))?;
        return Ok(calendar(ics.into_bytes()));
    }

    Ok(StatusCode::NOT_FOUND.into_response())
}

fn safe_join(root: &Path, request: &str) -> Option<PathBuf> {
    let relative = Path::new(request);
    if relative.is_absolute() {
        return None;
    }

    let mut path = root.to_path_buf();
    for component in relative.components() {
        match component {
            Component::Normal(part) => path.push(part),
            _ => return None,
        }
    }

    Some(path)
}

fn calendar(bytes: Vec<u8>) -> Response {
    (
        [
            (header::CONTENT_TYPE, HeaderValue::from_static(CONTENT_TYPE)),
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static(CACHE_CONTROL),
            ),
        ],
        bytes,
    )
        .into_response()
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
