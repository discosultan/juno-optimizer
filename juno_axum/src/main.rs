mod error;
mod exchange;
mod routes;

use anyhow::Result;
use axum::{routing::get, Router};
use juno::clients::juno_core;
use serde::Serialize;
use std::{env, net::SocketAddr, sync::Arc};
use tower_http::cors::CorsLayer;

#[derive(Serialize)]
struct ErrorResponse {
    message: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Set default log level to info.
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info")
    }
    // Install global collector configured based on RUST_LOG env var.
    tracing_subscriber::fmt::init();

    let juno_core_client = Arc::new(juno_core::Client::new("http://localhost:3030"));

    let app = Router::<Arc<juno_core::Client>>::new()
        .route("/", get(|| async { "hello world" }))
        .nest("/backtest", routes::backtest())
        .nest("/optimize", routes::optimize())
        .with_state(juno_core_client)
        .layer(CorsLayer::permissive());

    let addr = SocketAddr::from(([127, 0, 0, 1], 4040));
    tracing::info!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
