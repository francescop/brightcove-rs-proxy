use reqwest::header::ACCEPT;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const VIDEOS_PER_PAGE: u32 = 25;

#[derive(Serialize, Deserialize)]
pub struct AccessTokenResponse {
    access_token: String,
    token_type: String,
    expires_in: i64,
}

pub mod analytics {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
    pub struct VideosResponse {
        pub item_count: u32,
        pub items: Vec<Video>,
    }

    #[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
    pub struct Video {
        pub video: Option<String>,
        pub video_view: u32,
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct PlayerResponse {
    pub count: u32,
    pub videos: Vec<Video>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Video {
    pub id: String,
    pub name: String,
    pub thumbnail: String,
    pub custom_fields: VideoCustomFields,
    pub video_views: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct VideoCustomFields {
    pub numero_corsa: String,
    pub data: String,
    pub tipologia: Option<String>,
    pub cavalli: Option<String>,
    pub fantini: Option<String>,
    pub primo: Option<String>,
    pub secondo: Option<String>,
    pub terzo: Option<String>,
    pub ippodromo: String,
}

impl From<crate::db::VideoRow> for Video {
    fn from(video: crate::db::VideoRow) -> Self {
        Video {
            id: video.bc_video_id.clone(),
            name: video.name.clone(),
            thumbnail: video.thumbnail.clone(),
            custom_fields: VideoCustomFields {
                numero_corsa: video.numero_corsa.clone(),
                data: video.data.clone(),
                tipologia: video.tipologia.clone(),
                cavalli: video.cavalli.clone(),
                fantini: video.fantini.clone(),
                primo: video.primo.clone(),
                secondo: video.secondo.clone(),
                terzo: video.terzo.clone(),
                ippodromo: video.ippodromo.clone(),
            },
            video_views: Some(video.video_views),
        }
    }
}

impl From<&crate::db::VideoRow> for Video {
    fn from(video: &crate::db::VideoRow) -> Self {
        Video {
            id: video.bc_video_id.clone(),
            name: video.name.clone(),
            thumbnail: video.thumbnail.clone(),
            custom_fields: VideoCustomFields {
                numero_corsa: video.numero_corsa.clone(),
                data: video.data.clone(),
                tipologia: video.tipologia.clone(),
                cavalli: video.cavalli.clone(),
                fantini: video.fantini.clone(),
                primo: video.primo.clone(),
                secondo: video.secondo.clone(),
                terzo: video.terzo.clone(),
                ippodromo: video.ippodromo.clone(),
            },
            video_views: Some(video.video_views),
        }
    }
}

pub async fn get_access_token() -> String {
    let mut params = HashMap::new();
    params.insert("grant_type", "client_credentials");

    let client = reqwest::Client::new();
    let res = client
        .post("https://oauth.brightcove.com/v4/access_token")
        .basic_auth(dotenv!("CLIENT_ID"), Some(dotenv!("CLIENT_SECRET")))
        .form(&params)
        .send()
        .await
        .expect("error requesting token")
        .json::<AccessTokenResponse>()
        .await
        .expect("error deserializing response");

    res.access_token
}

pub(crate) async fn get_new_videos(
    latest_bc_video_id: Option<String>,
) -> anyhow::Result<Option<Vec<crate::db::VideoRow>>> {
    // create empty array `let bc_videos: Vec<XmlVideoAsset> = vec![];`
    // [bc api] get playable videos, sort by created_at, paginate 25
    let mut new_videos: Vec<crate::db::VideoRow> = Vec::new();

    let mut page: u32 = 1;

    let res = call_bc_player_url(page).await?;
    let max_pages = (res.count as f32 / VIDEOS_PER_PAGE as f32).ceil() as u32;

    log::debug!(target:"brightcove"," get_new_videos max_pages: {}", max_pages);

    let mut stop_processing = false;

    // - iterate over playable videos until video_id == latest_bc_video_id,
    //   adding videos to bc_videos

    // TODO: the final url is called two times, one to get max pages and the other to get the
    // videos. find a way to streamline it.
    while page <= max_pages && !stop_processing {
        log::debug!(target:"brightcove"," get_new_videos page {}/{}", page, max_pages);
        let res = call_bc_player_url(page).await?;

        match &latest_bc_video_id {
            Some(latest_video_id) => {
                for video in res.videos.iter() {
                    log::debug!(
                    target:"brightcove",
                                            " get_new_videos evaluating: {} == {}",
                                            &video.id,
                                            &latest_video_id,
                                        );
                    if &video.id != latest_video_id {
                        log::debug!(
                        target:"brightcove",
                                                    " get_new_videos added video to new_videos list: {}",
                                                    &video.id
                                                );
                        new_videos.push(video.into());
                    } else {
                        log::debug!(
                        target:"brightcove",
                                                    " get_new_videos latest video {} found on page {}. stop processing.",
                                                    &video.id,
                                                    page,
                                                );
                        stop_processing = true;
                        break;
                    }
                }
            }
            None => {
                log::debug!(target:"brightcove"," get_new_videos no videos in db, adding everything");
                for video in res.videos.iter() {
                    log::debug!(
                    target:"brightcove",
                                            " get_new_videos added video to new_videos list: {}",
                                            &video.id
                                        );
                    new_videos.push(video.into());
                }
            }
        }
        page += 1;
    }

    log::debug!(target:"brightcove"," get_new_videos passing {} videos", &new_videos.len());
    if !new_videos.is_empty() {
        Ok(Some(new_videos))
    } else {
        Ok(None)
    }
}

pub async fn call_bc_player_url(page: u32) -> anyhow::Result<PlayerResponse> {
    let offset = VIDEOS_PER_PAGE * (page - 1);
    let url = format!(
        "https://edge.api.brightcove.com/playback/v1/accounts/{}/videos?sort=-created_at&limit={}&offset={}",
        dotenv!("ACCOUNT_ID"),
        VIDEOS_PER_PAGE,
        offset
    );

    let accept_header = "application/json;pk=".to_string() + dotenv!("POLICY_KEY");

    let client = reqwest::Client::new();
    let res: PlayerResponse = client
        .get(&url)
        .header(ACCEPT, accept_header)
        .send()
        .await
        .expect("error requesting ")
        .json()
        .await
        .expect("error deserializing");

    log::debug!(
    target:"brightcove",
            "call_bc_player_url page {}, offset {}, url {}, first: {}, last: {}",
            page,
            offset,
            url,
            res.videos
                .iter()
                .map(|v| v.id.clone() + ", ")
                .collect::<String>(),
            res.videos.last().unwrap().id,
        );

    Ok(res)
}

pub async fn get_all_video_views(token: &str) -> anyhow::Result<analytics::VideosResponse> {
    let client = reqwest::Client::new();

    let url = format!(
            "https://analytics.api.brightcove.com/v1/data?accounts={}&limit=all&dimensions=video&fields=video_view",
            dotenv!("ACCOUNT_ID"),
        );

    let video_views_res = client.get(url).bearer_auth(token).send().await?;

    log::debug!(target:"brightcove","response: {:#?}", video_views_res);
    match video_views_res.status() {
        StatusCode::OK => {
            let body: analytics::VideosResponse = video_views_res.json().await?;
            log::debug!(target:"brightcove","body: {:#?}", body);

            // ignore video with NULL video id, fixes BC api
            let valid_videos: Vec<analytics::Video> = body
                .items
                .into_iter()
                .filter(|v| v.video.is_some())
                .collect();

            Ok(analytics::VideosResponse {
                item_count: valid_videos.len() as u32,
                items: valid_videos,
            })
        }

        status_code => {
            let body = &video_views_res.text().await.unwrap();
            panic!("Received response: {} {:#?}", status_code, body);
        }
    }
}

#[test]
fn deserialize_analytics_video() {
    let src = r#" {
	"item_count": 49,
	"items": [{
		"video": "v1_id",
		"video_view": 1
	}, {
		"video": "v2_id",
		"video_view": 2
	},{
		"video": null,
		"video_view": 2
	}],
	"summary": {
		"video_view": 187
	}
} "#;

    let item: analytics::VideosResponse = serde_json::from_str(src).unwrap();

    assert_eq!(
        item,
        analytics::VideosResponse {
            item_count: 49,
            items: vec![
                analytics::Video {
                    video: Some("v1_id".to_string()),
                    video_view: 1,
                },
                analytics::Video {
                    video: Some("v2_id".to_string()),
                    video_view: 2,
                },
                analytics::Video {
                    video: None,
                    video_view: 2,
                }
            ]
        }
    );
}
