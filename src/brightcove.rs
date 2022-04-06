use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
        pub video: String,
        pub video_view: u32,
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

pub async fn get_video_views(token: &str, video_ids_str: String) -> analytics::VideosResponse {
    let client = reqwest::Client::new();

    let url = format!(
        "https://analytics.api.brightcove.com/v1/data?accounts={}&dimensions=video&where=video=={}",
        dotenv!("ACCOUNT_ID"),
        video_ids_str
    );

    log::info!("{:#?}", url);

    let video_views_res = client
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .expect("error getting video views");

    log::debug!("response: {:#?}", video_views_res);
    match video_views_res.status() {
        StatusCode::OK => {
            let body: analytics::VideosResponse = video_views_res
                .json()
                .await
                .expect("error deserializing json response");
            log::debug!("body: {:#?}", body);
            body
        }

        _s => {
            let body = &video_views_res.text().await.unwrap();
            panic!("Received response: {:#?}", body);
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
                    video: "v1_id".to_string(),
                    video_view: 1,
                },
                analytics::Video {
                    video: "v2_id".to_string(),
                    video_view: 2,
                }
            ]
        }
    );
}
