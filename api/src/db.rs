use actix_web::{web, Result, error::ErrorInternalServerError, Error};
use serde::{Deserialize, Serialize};
use deadpool_postgres::{Pool, Client};
use tokio_postgres::types::ToSql;

// ── DB 마이그레이션 ───────────────────────────────────────────────────────────

pub async fn run_migrations(pool: &Pool) {
    let client = pool.get().await.expect("DB 연결 실패");
    client.batch_execute(r#"
        CREATE TABLE IF NOT EXISTS users (
            id SERIAL PRIMARY KEY,
            provider VARCHAR(20) NOT NULL,
            provider_id VARCHAR(100) NOT NULL,
            nickname VARCHAR(100) NOT NULL DEFAULT '',
            created_at TIMESTAMPTZ DEFAULT NOW(),
            UNIQUE(provider, provider_id)
        );

        ALTER TABLE users DROP COLUMN IF EXISTS email;
        ALTER TABLE users DROP COLUMN IF EXISTS profile_image;

        CREATE TABLE IF NOT EXISTS saved_locations (
            id SERIAL PRIMARY KEY,
            user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            name VARCHAR(200) NOT NULL,
            lat DOUBLE PRECISION NOT NULL,
            lng DOUBLE PRECISION NOT NULL,
            memo TEXT,
            created_at TIMESTAMPTZ DEFAULT NOW()
        );
    "#).await.expect("마이그레이션 실패");
    println!("DB 마이그레이션 완료");
}

// ── 사용자 ────────────────────────────────────────────────────────────────────

pub async fn upsert_user(
    pool: &web::Data<Pool>,
    provider: &str,
    provider_id: &str,
    nickname: &str,
) -> Result<i32, Box<dyn std::error::Error>> {
    let client: Client = pool.get().await?;
    let row = client.query_one(
        r#"
        INSERT INTO users (provider, provider_id, nickname)
        VALUES ($1, $2, $3)
        ON CONFLICT (provider, provider_id) DO UPDATE
            SET nickname = EXCLUDED.nickname
        RETURNING id
        "#,
        &[&provider, &provider_id, &nickname],
    ).await?;
    Ok(row.get(0))
}

// ── 저장 위치 ─────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Debug)]
pub struct SavedLocation {
    pub id: i32,
    pub user_id: i32,
    pub name: String,
    pub lat: f64,
    pub lng: f64,
    pub memo: Option<String>,
    pub created_at: String,
}

#[derive(Deserialize, Debug)]
pub struct CreateLocationInput {
    pub name: String,
    pub lat: f64,
    pub lng: f64,
    pub memo: Option<String>,
}

pub async fn create_location(
    pool: &web::Data<Pool>,
    user_id: i32,
    input: &CreateLocationInput,
) -> Result<SavedLocation, Box<dyn std::error::Error>> {
    let client: Client = pool.get().await?;
    let row = client.query_one(
        "INSERT INTO saved_locations (user_id, name, lat, lng, memo) VALUES ($1, $2, $3, $4, $5) RETURNING id, user_id, name, lat, lng, memo, to_char(created_at, 'YYYY-MM-DD HH24:MI')",
        &[&user_id, &input.name, &input.lat, &input.lng, &input.memo],
    ).await?;
    Ok(SavedLocation {
        id: row.get(0),
        user_id: row.get(1),
        name: row.get(2),
        lat: row.get(3),
        lng: row.get(4),
        memo: row.get(5),
        created_at: row.get(6),
    })
}

pub async fn get_locations(
    pool: &web::Data<Pool>,
    user_id: i32,
) -> Result<Vec<SavedLocation>, Box<dyn std::error::Error>> {
    let client: Client = pool.get().await?;
    let rows = client.query(
        "SELECT id, user_id, name, lat, lng, memo, to_char(created_at, 'YYYY-MM-DD HH24:MI') FROM saved_locations WHERE user_id = $1 ORDER BY created_at DESC",
        &[&user_id],
    ).await?;
    Ok(rows.iter().map(|r| SavedLocation {
        id: r.get(0),
        user_id: r.get(1),
        name: r.get(2),
        lat: r.get(3),
        lng: r.get(4),
        memo: r.get(5),
        created_at: r.get(6),
    }).collect())
}

pub async fn delete_location(
    pool: &web::Data<Pool>,
    location_id: i32,
    user_id: i32,
) -> Result<bool, Box<dyn std::error::Error>> {
    let client: Client = pool.get().await?;
    let rows = client.execute(
        "DELETE FROM saved_locations WHERE id = $1 AND user_id = $2",
        &[&location_id, &user_id],
    ).await?;
    Ok(rows > 0)
}

// API 응답으로 보낼 데이터의 구조체입니다.
#[derive(Serialize, Deserialize, Debug)]
pub struct MapData {
    pub ogc_fid: i32,
    pub wkb_geometry: Option<String>,
    pub write_year: Option<String>,
    pub agcls_cd: Option<String>,
}

// POST 요청으로 받을 데이터의 구조체입니다.
#[derive(Serialize, Deserialize, Debug)]
pub struct CreateChamnamuData {
    pub wkb_geometry: Option<String>,
    pub write_year: Option<String>,
    pub agcls_cd: Option<String>,
}

// 모든 맵 데이터를 조회하는 함수입니다.
pub async fn get_all_polygon_data(pool: web::Data<Pool>) -> Result<Vec<MapData>, Error> {
    let client: Client = pool.get().await.map_err(ErrorInternalServerError)?;

    let rows = client.query("SELECT ogc_fid, ST_AsText(ST_Transform(wkb_geometry, 4326)), 갱신년도, agcls_cd FROM chamnamu_tree", &[])
        .await
        .map_err(ErrorInternalServerError)?;
        
    let map_data_list: Vec<MapData> = rows.iter().map(|row| {
        MapData {
            ogc_fid: row.get(0),
            wkb_geometry: row.get(1),
            write_year: row.get(2),
            agcls_cd: row.get(3),
        }
    }).collect();
    
    Ok(map_data_list)
}

// 새로운 맵 데이터를 생성하는 함수입니다.
pub async fn create_new_chamnamu_data(pool: web::Data<Pool>, new_data: CreateChamnamuData) -> Result<i32, Error> {
    let client: Client = pool.get().await.map_err(ErrorInternalServerError)?;

    println!("새로운 데이터 생성 요청: {:#?}", new_data);

    let rows = client
        .query(
            "INSERT INTO chamnamu_tree (wkb_geometry, 갱신년도, agcls_cd) VALUES (ST_GeomFromText($1), $2, $3) RETURNING ogc_fid",
            &[&new_data.wkb_geometry as &(dyn ToSql + Sync), &new_data.write_year, &new_data.agcls_cd],
        )
        .await
        .map_err(ErrorInternalServerError)?;
    
    let id: i32 = rows[0].get(0);
    println!("새로 생성된 ogc_fid: {}", id);
    
    Ok(id)
}

// 특정 좌표 주변의 맵 데이터를 조회하는 함수입니다.
// wkt_point: 'POINT(x y)' 형식의 문자열, distance: 미터 단위의 거리
pub async fn get_nearby_polygon_data(pool: web::Data<Pool>, lng: f64, lat:f64, distance: f64) -> Result<Vec<MapData>, Error> {
    let client: Client = pool.get().await.map_err(ErrorInternalServerError)?;
    let query = r#"
        SELECT ogc_fid, ST_AsText(ST_Transform(wkb_geometry, 4326)), 갱신년도, agcls_cd
        FROM chamnamu_tree
        WHERE ST_DWithin(
            wkb_geometry,
            ST_Transform(ST_SetSRID(ST_MakePoint($1, $2), 4326), 5179),
            $3
        )
    "#;

    let rows = client.query(query, &[&lng, &lat, &distance])
        .await
        .map_err(ErrorInternalServerError)?;

    let map_data_list: Vec<MapData> = rows.iter().map(|row| {
        MapData {
            ogc_fid: row.get(0),
            wkb_geometry: row.get(1),
            write_year: row.get(2),
            agcls_cd: row.get(3),
        }
    }).collect();

    Ok(map_data_list)
}