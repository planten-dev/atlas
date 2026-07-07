mod config;
mod dto;
mod handlers;

use axum::{Router, routing::get};
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let config = config::load().expect("failed to load application config");
    let app = Router::new().route("/health", get(handlers::health));
    let addr: SocketAddr = config
        .server
        .bind_addr
        .parse()
        .expect("server.bind_addr must be a valid socket address");
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind server address");

    axum::serve(listener, app)
        .await
        .expect("server exited unexpectedly");
}
