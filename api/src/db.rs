use actix_web::{web, Result, error::ErrorInternalServerError, Error};
use serde::{Deserialize, Serialize};
use deadpool_postgres::{Pool, Client};
use tokio_postgres::types::ToSql;
use serde_json::value::RawValue;

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
#[derive(Serialize, Debug)]
pub struct MapData {
    pub id: i32,
    pub geometry: Box<RawValue>,
    pub species: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct NearestPolygon {
    pub id: i32,
    pub species: Option<String>,
    pub lat: f64,
    pub lng: f64,
    pub distance_m: f64,
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

    let rows = client.query("SELECT ogc_fid, ST_AsGeoJSON(ST_Transform(wkb_geometry, 4326), 6), koftr_nm FROM chamnamu_tree", &[])
        .await
        .map_err(ErrorInternalServerError)?;

    let map_data_list: Vec<MapData> = rows.iter().map(|row| {
        let geom_str: String = row.get(1);
        MapData {
            id: row.get(0),
            geometry: RawValue::from_string(geom_str).unwrap_or_else(|_| RawValue::from_string("null".to_string()).unwrap()),
            species: row.get(2),
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

// 현재 지도 뷰포트(경계 상자) 안에 들어오는 맵 데이터를 조회하는 함수입니다.
// 위/경도 기준 bbox가 너무 크면(너무 축소된 상태) 빈 목록을 반환합니다 - 프론트에서 "확대해주세요" 안내로 처리.
const MAX_BBOX_DEGREES: f64 = 1.0; // 위/경도 한 변 기준 대략 110km 내외 (지도 maxLevel:7까지는 넉넉히 커버)

// 뷰포트 안 폴리곤이 아무리 많아도 응답이 무한정 커지지 않도록 잡아두는 안전 상한.
const MAX_ROW_LIMIT: i64 = 40_000;

pub async fn get_polygons_in_bbox(
    pool: web::Data<Pool>,
    min_lng: f64, min_lat: f64, max_lng: f64, max_lat: f64,
) -> Result<(Vec<MapData>, f64), Error> {
    if (max_lng - min_lng) > MAX_BBOX_DEGREES || (max_lat - min_lat) > MAX_BBOX_DEGREES {
        return Ok((vec![], 0.0));
    }

    let client: Client = pool.get().await.map_err(ErrorInternalServerError)?;

    // 개수 기반 단순화(ST_Simplify) 없이 항상 원본 geometry를 그대로 반환한다.
    // ORDER BY로 매 요청 동일한 부분집합이 뽑히게 해 안전 상한 초과 시에도 화면이 깜빡이지 않게 한다.
    let rows = client.query(
        &format!(
            r#"
            SELECT ogc_fid, ST_AsGeoJSON(ST_Transform(wkb_geometry, 4326), 6), koftr_nm
            FROM chamnamu_tree
            WHERE wkb_geometry && ST_Transform(ST_MakeEnvelope($1, $2, $3, $4, 4326), 5179)
            ORDER BY ogc_fid
            LIMIT {MAX_ROW_LIMIT}
            "#
        ),
        &[&min_lng, &min_lat, &max_lng, &max_lat],
    ).await.map_err(ErrorInternalServerError)?;

    let map_data_list: Vec<MapData> = rows.iter().filter_map(|row| {
        let geom_str: Option<String> = row.get(1);
        geom_str.map(|geom_str| MapData {
            id: row.get(0),
            geometry: RawValue::from_string(geom_str).unwrap_or_else(|_| RawValue::from_string("null".to_string()).unwrap()),
            species: row.get(2),
        })
    }).collect();

    Ok((map_data_list, 0.0))
}

pub async fn get_nearest_polygon_by_species(
    pool: web::Data<Pool>,
    lng: f64,
    lat: f64,
    species: &str,
    excluded_ids: &[i32],
) -> Result<Option<NearestPolygon>, Error> {
    let client: Client = pool.get().await.map_err(ErrorInternalServerError)?;
    let excluded_ids = excluded_ids.to_vec();
    let query = r#"
        WITH origin AS (
            SELECT ST_Transform(ST_SetSRID(ST_MakePoint($1, $2), 4326), 5179) AS geom
        ),
        candidates AS MATERIALIZED (
            SELECT tree.ogc_fid, tree.koftr_nm, tree.wkb_geometry, origin.geom AS origin_geom
            FROM chamnamu_tree AS tree
            CROSS JOIN origin
            WHERE tree.koftr_nm = $3
              AND NOT (tree.ogc_fid = ANY($4))
            ORDER BY tree.wkb_geometry <-> origin.geom
            LIMIT 64
        )
        SELECT
            ogc_fid,
            koftr_nm,
            ST_Y(ST_Transform(ST_ClosestPoint(wkb_geometry, origin_geom), 4326)) AS lat,
            ST_X(ST_Transform(ST_ClosestPoint(wkb_geometry, origin_geom), 4326)) AS lng,
            ST_Distance(wkb_geometry, origin_geom) AS distance_m
        FROM candidates
        ORDER BY distance_m
        LIMIT 1
    "#;

    let row = client.query_opt(query, &[&lng, &lat, &species, &excluded_ids])
        .await
        .map_err(ErrorInternalServerError)?;

    Ok(row.map(|row| NearestPolygon {
        id: row.get(0),
        species: row.get(1),
        lat: row.get(2),
        lng: row.get(3),
        distance_m: row.get(4),
    }))
}
