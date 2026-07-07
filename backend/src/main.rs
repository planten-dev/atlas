mod dto;
mod handlers;

use axum::{Router, routing::get};
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let app = Router::new().route("/health", get(handlers::health));
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind server address");

    axum::serve(listener, app)
        .await
        .expect("server exited unexpectedly");
}
