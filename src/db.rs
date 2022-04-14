use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, sqlx::FromRow)]
pub struct VideoRow {
    pub name: String,
    pub thumbnail: String,
    pub numero_corsa: String,
    pub data: String,
    pub tipologia: Option<String>,
    pub cavalli: Option<String>,
    pub fantini: Option<String>,
    pub primo: Option<String>,
    pub secondo: Option<String>,
    pub terzo: Option<String>,
    pub ippodromo: String,
    pub video_views: u32,
    pub bc_video_id: String,
}

impl From<&crate::brightcove::Video> for VideoRow {
    fn from(video: &crate::brightcove::Video) -> Self {
        VideoRow {
            name: video.name.clone(),
            thumbnail: video.thumbnail.clone(),
            bc_video_id: video.id.clone(),
            numero_corsa: video.custom_fields.numero_corsa.clone(),
            data: video.custom_fields.data.clone(),
            tipologia: video.custom_fields.tipologia.clone(),
            cavalli: video.custom_fields.cavalli.clone(),
            fantini: video.custom_fields.fantini.clone(),
            primo: video.custom_fields.primo.clone(),
            secondo: video.custom_fields.secondo.clone(),
            terzo: video.custom_fields.terzo.clone(),
            ippodromo: video.custom_fields.ippodromo.clone(),
            video_views: 0,
        }
    }
}

pub async fn update_video_views(
    conn: &mut sqlx::pool::PoolConnection<sqlx::Sqlite>,
    bc_video_id: &str,
    views: &u32,
) -> anyhow::Result<bool> {
    let query_str = format!(
        "UPDATE videos SET video_views = {} WHERE bc_video_id = {}",
        views, bc_video_id
    );

    let query = sqlx::query(&query_str);

    let rows_affected = query.execute(conn).await?.rows_affected();

    Ok(rows_affected > 0)
}

pub async fn get_latest_bc_video_id(
    //pool: &sqlx::Pool<sqlx::Sqlite>,
    conn: &mut sqlx::pool::PoolConnection<sqlx::Sqlite>,
) -> anyhow::Result<Option<String>> {
    let row: (String,) = sqlx::query_as(
        r#"
            SELECT bc_video_id
            FROM videos
            ORDER BY bc_video_id
            DESC LIMIT 1
        "#,
    )
    .fetch_one(conn)
    .await?;

    log::debug!(target:"db", "get_latest_bc_video_id: {}", row.0);

    Ok(Some(row.0))
}

pub(crate) async fn save_videos(
    conn: &mut sqlx::pool::PoolConnection<sqlx::Sqlite>,
    new_videos: &Vec<VideoRow>,
) -> anyhow::Result<()> {
    for video in new_videos {
        let id = sqlx::query(
            r#"
        INSERT INTO videos (
            name,
            thumbnail,
            numero_corsa,
            data,
            tipologia,
            cavalli,
            fantini,
            primo,
            secondo,
            terzo,
            ippodromo,
            video_views,
            bc_video_id
 )
        VALUES (
            ?,
            ?,
            ?,
            ?,
            ?,
            ?,
            ?,
            ?,
            ?,
            ?,
            ?,
            ?,
            ?
  )
        "#,
        )
        .bind(&video.name)
        .bind(&video.thumbnail)
        .bind(&video.numero_corsa)
        .bind(&video.data)
        .bind(&video.tipologia)
        .bind(&video.cavalli)
        .bind(&video.fantini)
        .bind(&video.primo)
        .bind(&video.secondo)
        .bind(&video.terzo)
        .bind(&video.ippodromo)
        .bind(&video.video_views)
        .bind(&video.bc_video_id)
        .execute(&mut *conn)
        .await?
        .last_insert_rowid();

        log::debug!(target:"db", "saved video: {} - {}", id, &video.bc_video_id);
    }

    Ok(())
}

pub(crate) async fn get_videos(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    limit: &u32,
    offset: &u32,
) -> crate::brightcove::PlayerResponse {
    let mut conn = pool.acquire().await.unwrap();

    let (count,): (u32,) = sqlx::query_as("SELECT COUNT(*) FROM videos")
        .fetch_one(&mut conn)
        .await
        .unwrap();

    let video_rows: Vec<VideoRow> = sqlx::query_as(
        r#"
            select
                name,
                thumbnail,
                numero_corsa,
                data,
                tipologia,
                cavalli,
                fantini,
                primo,
                secondo,
                terzo,
                ippodromo,
                video_views,
                bc_video_id
            from videos
            ORDER BY data DESC LIMIT ? OFFSET ?
        "#,
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&mut conn)
    .await
    .unwrap();

    let videos: Vec<crate::brightcove::Video> = video_rows.iter().map(|v| v.into()).collect();

    crate::brightcove::PlayerResponse { count, videos }
}

pub(crate) async fn get_video(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    video_id: &str,
) -> crate::brightcove::Video {
    let mut conn = pool.acquire().await.unwrap();
    let video_row: VideoRow = sqlx::query_as(
        r#"
            select
                name,
                thumbnail,
                numero_corsa,
                data,
                tipologia,
                cavalli,
                fantini,
                primo,
                secondo,
                terzo,
                ippodromo,
                video_views,
                bc_video_id
            from videos
            where bc_video_id = ?
        "#,
    )
    .bind(video_id)
    .fetch_one(&mut conn)
    .await
    .unwrap();

    let video: crate::brightcove::Video = video_row.into();
    video
}

/*
pub(crate) async fn get_all_video_ids(
    conn: &mut sqlx::pool::PoolConnection<sqlx::Sqlite>,
) -> anyhow::Result<Vec<String>> {
    #[derive(Debug, Clone, sqlx::FromRow)]
    struct VideoId {
        bc_video_id: String,
    }

    let video_ids: Vec<VideoId> = sqlx::query_as(
        r#"
            select
                bc_video_id as "String"
            from videos
        "#,
    )
    .fetch_all(&mut *conn)
    .await
    .unwrap();

    Ok(video_ids.iter().map(|v| v.bc_video_id.clone()).collect())
}
*/
