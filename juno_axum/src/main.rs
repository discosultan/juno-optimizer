mod routes;

use anyhow::Result;
use axum::{
    http::{self, HeaderValue, Method},
    routing::get,
    Extension, Router,
};
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

    let app = Router::new()
        .layer(
            CorsLayer::new()
                .allow_origin("*".parse::<HeaderValue>()?)
                .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
                .allow_headers([http::header::CONTENT_TYPE]),
        )
        .route("/", get(|| async { "hello world" }))
        .nest("/backtest", routes::backtest())
        .nest("/optimize", routes::optimize())
        .layer(Extension(juno_core_client));

    let addr = SocketAddr::from(([127, 0, 0, 1], 4040));
    tracing::info!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
