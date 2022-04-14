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

use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use http::Method;

use std::sync::Arc;
mod brightcove;
mod db;

use sqlx::sqlite::SqlitePool;

use std::time::Duration;
use tokio::{sync::Mutex, task, time};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();

    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "info,tower_http=debug")
    }

    tracing_subscriber::fmt::init();

    let ro_pool = SqlitePool::connect(dotenv!("DATABASE_URL")).await?;
    let rw_pool = SqlitePool::connect(dotenv!("DATABASE_URL")).await?;
    sqlx::migrate!().run(&rw_pool).await?;

    let brightcove_access_token = Arc::new(Mutex::new(brightcove::get_access_token().await));
    let brightcove_access_token_for_thread = brightcove_access_token.clone();
    let brightcove_access_token_for_video_views_thread = brightcove_access_token.clone();

    let rw_pool = Arc::new(Mutex::new(rw_pool));
    let video_sync_pool = rw_pool.clone();
    let video_views_pool = rw_pool.clone();

    let thread_get_access_token_interval = dotenv!("THREAD_GET_ACCESS_TOKEN_DELAY_IN_S")
        .parse::<u64>()
        .unwrap();

    let thread_sync_video_interval = dotenv!("THREAD_SYNC_VIDEO_DELAY_IN_S")
        .parse::<u64>()
        .unwrap();

    let thread_sync_views_interval = dotenv!("THREAD_SYNC_VIEWS_DELAY_IN_S")
        .parse::<u64>()
        .unwrap();

    // thread that gets access token
    task::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(thread_get_access_token_interval));

        loop {
            interval.tick().await;
            log::info!(target: "get_access_token", "time expired, getting new token");
            let new_brightcove_access_token = brightcove::get_access_token().await;

            let mut brightcove_access_token = brightcove_access_token_for_thread.lock().await;
            *brightcove_access_token = new_brightcove_access_token;
        }
    });

    // thread that syncs videos
    task::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(thread_sync_video_interval));

        loop {
            log::info!(target: "sync videos", "checking new videos");

            // TODO: cosa succede se si elimina un video da BC? come lo si risincronizza?

            /*
             * - get latest_bc_video_id from db
             * - create empty array `let bc_videos: Vec<XmlVideoAsset> = vec![];`
             * - [bc api] get playable videos, sort by created_at, paginate 25
             * - iterate over playable videos until video_id == latest_bc_video_id,
             *   adding videos to bc_videos
             *
             * - for each bc_videos create database row
             *
             */

            // get latest_bc_video_id from db
            let mut conn = video_sync_pool.lock().await.acquire().await.unwrap();
            let latest_bc_video_id = db::get_latest_bc_video_id(&mut conn).await;

            let latest_bc_video_id = match latest_bc_video_id {
                Ok(id) => {
                    log::info!(target: "sync videos","latest_bc_video_id: {}", &id.as_ref().unwrap());
                    id
                }
                _ => {
                    log::info!(target: "sync videos","info: no latest_bc_video_id");
                    None
                }
            };

            // create empty array `let bc_videos: Vec<XmlVideoAsset> = vec![];`
            // [bc api] get playable videos, sort by created_at, paginate 25
            let new_videos_result = brightcove::get_new_videos(latest_bc_video_id.clone()).await;

            if let Ok(new_videos) = new_videos_result {
                match new_videos {
                    Some(new_videos) => {
                        log::debug!(target: "sync videos"," saving {} new videos", &new_videos.len());

                        match db::save_videos(&mut conn, &new_videos).await {
                            Ok(_) => {
                                log::info!(target: "sync videos"," saved {} new videos", &new_videos.len())
                            }
                            Err(e) => log::error!(target: "sync videos",
                                " fail saved {} new videos: {}",
                                &new_videos.len(),
                                e.to_string()
                            ),
                        }
                    }
                    None => {
                        log::info!(target: "sync videos"," no new videos");
                    }
                }
            } else {
                log::info!(target: "sync videos"," no new videos");
            };
            interval.tick().await;
        }
    });

    // thread that syncs views
    task::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(thread_sync_views_interval));

        loop {
            let mut conn = video_views_pool.lock().await.acquire().await.unwrap();

            let brightcove_access_token =
                brightcove_access_token_for_video_views_thread.lock().await;

            let video_views_response: anyhow::Result<brightcove::analytics::VideosResponse> =
                brightcove::get_all_video_views(&brightcove_access_token).await;

            match video_views_response {
                Ok(response) => {
                    if response.item_count > 0 {
                        for item in response.items {
                            match &item.video {
                                Some(video) => {
                                    match db::update_video_views(&mut conn, video, &item.video_view)
                                        .await
                                    {
                                        Ok(_) => {
                                            log::debug!(
                                                target: "sync views",
                                                "updated views for video: {:?}",
                                                &item.video
                                            )
                                        }
                                        _ => log::debug!(
                                        target: "sync views",
                                                                                    "error updating video: {:?}",
                                                                                    &item.video
                                                                                ),
                                    };
                                }
                                None => {}
                            }
                        }
                        // update record
                    }
                }
                Err(e) => log::error!(
                   target: "sync views", "{}", e),
            }
            interval.tick().await;
        }
    });

    let cors = CorsLayer::new()
        .allow_methods(vec![Method::GET])
        .allow_headers(vec![http::header::CONTENT_TYPE])
        .allow_headers(vec![http::header::ACCEPT])
        .allow_origin(Any);

    let routes = Router::new()
        .route("/videos", get(videos_index))
        .route("/videos/:video_id", get(video_show))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .layer(Extension(ro_pool));

    let app = Router::new().nest("/api/v1", routes);

    axum::Server::bind(&"0.0.0.0:4000".parse()?)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

async fn videos_index(
    pool: Extension<SqlitePool>,
    params: Query<HashMap<String, u32>>,
    _search_params: Option<Query<HashMap<String, String>>>,
) -> Json<brightcove::PlayerResponse> {
    let limit = match params.get("limit") {
        Some(l) => {
            if l > &20 {
                20_u32
            } else {
                *l
            }
        }
        None => 20,
    };
    let offset = match params.get("offset") {
        Some(offset) => *offset,
        None => 0,
    };

    // TODO:
    // - in mem sqlite
    // - multithreaded sqlite

    //let videos = db::get_videos(&pool, &limit, &offset, &date, &cavalli).await;
    let videos = db::get_videos(&pool, &limit, &offset).await;

    Json(videos)
}

async fn video_show(
    pool: Extension<SqlitePool>,
    video_id: Path<String>,
) -> Json<brightcove::Video> {
    let video = db::get_video(&pool, &video_id).await;

    Json(video)
}

#[test]
fn deserialize_player_response() {
    let src = r#"
    {
        "count": 444,
        "videos": [
        {
            "thumbnail": "https://.../image.jpg",
            "custom_fields": {
                "categorie": "FIT",
                "cavalli": "CLELIA DEI DALTRI,CICLONE TAV",
                "data": "2022/03/20",
                "fantini": "C.PISCUOGLIO,A.DI NARDO,E.BELLEI,E.LOCCISANO",
                "ippodromo": "FIRENZE",
                "numero_corsa": "02",
                "primo": "CASPIAN PLAY FONT",
                "secondo": "CICLONE TAV",
                "terzo": "CAPITAN SPAV",
                "tipologia": "TROTTO"
            },
            "name": "PR. LURABO BLUE",
            "id": "6301819598001",
                "video_views": 1
        }
        ]
    }
"#;

    let video_response: brightcove::PlayerResponse = serde_json::from_str(src).unwrap();

    assert_eq!(
        video_response,
        brightcove::PlayerResponse {
            count: 444,
            videos: vec![brightcove::Video {
                id: "6301819598001".to_string(),
                name: "PR. LURABO BLUE".to_string(),
                thumbnail: "https://.../image.jpg".to_string(),
                custom_fields: brightcove::VideoCustomFields {
                    numero_corsa: "02".to_string(),
                    data: "2022/03/20".to_string(),
                    tipologia: Some("TROTTO".to_string()),
                    cavalli: Some("CLELIA DEI DALTRI,CICLONE TAV".to_string()),
                    fantini: Some("C.PISCUOGLIO,A.DI NARDO,E.BELLEI,E.LOCCISANO".to_string()),
                    primo: Some("CASPIAN PLAY FONT".to_string()),
                    secondo: Some("CICLONE TAV".to_string()),
                    terzo: Some("CAPITAN SPAV".to_string()),
                    ippodromo: "FIRENZE".to_string(),
                },
                video_views: 1
            }]
        }
    );
}
