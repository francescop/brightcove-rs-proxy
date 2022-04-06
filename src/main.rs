#[macro_use]
extern crate dotenv_codegen;

use axum::{
    extract::{Extension, Path, Query},
    response::Json,
    routing::get,
    Router,
};
use std::collections::HashMap;

use dotenv::dotenv;
use std::env;

use serde_json::{json, Value};
use tower_http::cors::{Any, CorsLayer};
use tower_http::{classify::ServerErrorsFailureClass, trace::TraceLayer};

use serde::{Deserialize, Serialize};

use http::{header, Method, Request, Response};

use std::sync::Arc;
mod brightcove;

use std::time::Duration;
use tokio::{sync::Mutex, task, time};

use tracing::Span;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    dotenv().ok();

    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "info,tower_http=debug")
    }

    tracing_subscriber::fmt::init();

    let brightcove_access_token = Arc::new(Mutex::new(brightcove::get_access_token().await));
    let brightcove_access_token_for_thread = brightcove_access_token.clone();

    task::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(240)); // 4 minutes

        loop {
            interval.tick().await;
            log::info!("[main] time expired, getting new token");
            let new_brightcove_access_token = brightcove::get_access_token().await;

            let mut brightcove_access_token = brightcove_access_token_for_thread.lock().await;
            *brightcove_access_token = new_brightcove_access_token;
        }
    });

    let cors = CorsLayer::new()
        .allow_methods(vec![Method::GET])
        .allow_headers(vec![http::header::CONTENT_TYPE])
        .allow_headers(vec![http::header::ACCEPT])
        .allow_origin(Any);

    let analytics_routes = Router::new()
        .route("/analytics", get(get_video_views))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .layer(Extension(brightcove_access_token.clone()));

    let app = Router::new().nest("/api/v1", analytics_routes);

    axum::Server::bind(&"0.0.0.0:4000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

#[axum_debug::debug_handler]
async fn get_video_views(
    token: Extension<Arc<Mutex<String>>>,
    params: Query<HashMap<String, String>>,
) -> Json<brightcove::analytics::VideosResponse> {
    match params.get("videos") {
        Some(video_ids_str) => {
            let video_views_response: brightcove::analytics::VideosResponse =
                brightcove::get_video_views(&token.lock().await, video_ids_str.to_string()).await;
            Json(video_views_response)
        }
        None => {
            let res = brightcove::analytics::VideosResponse {
                item_count: 0,
                items: vec![],
            };

            Json(res)
        }
    }
}
