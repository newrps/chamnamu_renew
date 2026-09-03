use actix_web::{delete, get, middleware::Compress, post, web, App, HttpRequest, HttpResponse, HttpServer, Result, error::ErrorBadRequest};
use deadpool_postgres::{Pool, Manager};
use tokio_postgres::NoTls;
use dotenvy::dotenv;
use std::env;
use std::sync::Arc;
use tokio::sync::RwLock;
use actix_cors::Cors;
use serde::Deserialize;

mod db;
mod coupang;
mod auth;

type AdCache = Arc<RwLock<Vec<coupang::AdItem>>>;

// ── 폴리곤 ────────────────────────────────────────────────────────────────────

#[get("/api/polygon/nearby")]
async fn get_nearby_data(
    pool: web::Data<Pool>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse> {
    fn parse_f64(query: &std::collections::HashMap<String, String>, key: &str) -> Result<f64> {
        let raw = query.get(key).ok_or_else(|| ErrorBadRequest(format!("Missing {key}")))?;
        raw.parse::<f64>().map_err(|_| ErrorBadRequest(format!("Invalid {key}")))
    }

    let min_lng = parse_f64(&query, "minLng")?;
    let min_lat = parse_f64(&query, "minLat")?;
    let max_lng = parse_f64(&query, "maxLng")?;
    let max_lat = parse_f64(&query, "maxLat")?;
    // 더 이상 서버에서 단순화하지 않고 항상 원본 geometry를 반환한다 - 헤더는 과거 버전과의
    // 호환(프론트 캐시 키 포맷)을 위해 유지하되 값은 항상 0이다.
    let (list, tolerance_m) = db::get_polygons_in_bbox(pool, min_lng, min_lat, max_lng, max_lat).await?;
    Ok(HttpResponse::Ok()
        .insert_header(("X-Simplify-Tolerance", tolerance_m.to_string()))
        .json(list))
}

#[derive(Deserialize)]
struct NearestPolygonQuery {
    species: String,
    lat: f64,
    lng: f64,
    #[serde(rename = "excludeIds")]
    exclude_ids: Option<String>,
}

#[get("/api/polygon/nearest")]
async fn get_nearest_polygon(
    pool: web::Data<Pool>,
    query: web::Query<NearestPolygonQuery>,
) -> Result<HttpResponse> {
    if query.species.trim().is_empty() || query.species.chars().count() > 50 {
        return Err(ErrorBadRequest("Invalid species"));
    }
    if !(-90.0..=90.0).contains(&query.lat) || !(-180.0..=180.0).contains(&query.lng) {
        return Err(ErrorBadRequest("Invalid coordinates"));
    }

    let excluded_ids = match query.exclude_ids.as_deref() {
        Some(raw) if !raw.is_empty() => {
            let parsed = raw.split(',')
                .map(str::parse::<i32>)
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|_| ErrorBadRequest("Invalid excludeIds"))?;
            if parsed.len() > 100 {
                return Err(ErrorBadRequest("Too many excludeIds"));
            }
            parsed
        }
        _ => vec![],
    };

    match db::get_nearest_polygon_by_species(
        pool,
        query.lng,
        query.lat,
        query.species.trim(),
        &excluded_ids,
    ).await? {
        Some(item) => Ok(HttpResponse::Ok().json(item)),
        None => Ok(HttpResponse::NotFound().finish()),
    }
}

// ── 광고 ──────────────────────────────────────────────────────────────────────

#[get("/api/ads")]
async fn get_ads(cache: web::Data<AdCache>) -> HttpResponse {
    let ads = cache.read().await;
    // 가드를 직렬화 동안만 잡고, Vec 자체는 클론하지 않음 (메모리 사용량 ↓)
    HttpResponse::Ok().json(&*ads)
}

// ── 저장 위치 API ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct CreateLocationBody {
    name: String,
    lat: f64,
    lng: f64,
    memo: Option<String>,
}

#[post("/api/locations")]
async fn create_location(
    req: HttpRequest,
    pool: web::Data<Pool>,
    body: web::Json<CreateLocationBody>,
) -> HttpResponse {
    let claims = match auth::extract_token(&req) {
        Some(c) => c,
        None => return HttpResponse::Unauthorized().body("로그인이 필요합니다"),
    };
    let input = db::CreateLocationInput {
        name: body.name.clone(),
        lat: body.lat,
        lng: body.lng,
        memo: body.memo.clone(),
    };
    match db::create_location(&pool, claims.sub, &input).await {
        Ok(loc) => HttpResponse::Created().json(loc),
        Err(e) => { eprintln!("[위치 저장] error: {}", e); HttpResponse::InternalServerError().finish() }
    }
}

#[get("/api/locations")]
async fn get_locations(req: HttpRequest, pool: web::Data<Pool>) -> HttpResponse {
    let claims = match auth::extract_token(&req) {
        Some(c) => c,
        None => return HttpResponse::Unauthorized().body("로그인이 필요합니다"),
    };
    match db::get_locations(&pool, claims.sub).await {
        Ok(locs) => HttpResponse::Ok().json(locs),
        Err(e) => { eprintln!("[위치 조회] error: {}", e); HttpResponse::InternalServerError().finish() }
    }
}

#[delete("/api/locations/{id}")]
async fn delete_location(
    req: HttpRequest,
    pool: web::Data<Pool>,
    path: web::Path<i32>,
) -> HttpResponse {
    let claims = match auth::extract_token(&req) {
        Some(c) => c,
        None => return HttpResponse::Unauthorized().body("로그인이 필요합니다"),
    };
    let location_id = path.into_inner();
    match db::delete_location(&pool, location_id, claims.sub).await {
        Ok(true) => HttpResponse::Ok().finish(),
        Ok(false) => HttpResponse::NotFound().finish(),
        Err(e) => { eprintln!("[위치 삭제] error: {}", e); HttpResponse::InternalServerError().finish() }
    }
}

// ── main ──────────────────────────────────────────────────────────────────────

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();

    // 누락되거나 짧은 JWT 비밀키로 서버가 실행되지 않도록 시작 시점에 확인
    auth::ensure_jwt_secret();

    // DB
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let config = database_url.parse::<tokio_postgres::Config>().expect("Failed to parse DATABASE_URL");
    let manager = Manager::new(config, NoTls);
    let pool = Pool::builder(manager).build().unwrap();

    // DB 마이그레이션 (users, saved_locations 테이블 생성)
    db::run_migrations(&pool).await;

    // 쿠팡 광고
    let access_key = env::var("COUPANG_ACCESS_KEY").unwrap_or_default();
    let secret_key = env::var("COUPANG_SECRET_KEY").unwrap_or_default();
    let keywords: Vec<String> = env::var("COUPANG_KEYWORDS")
        .unwrap_or_else(|_| "사슴벌레,채집망,곤충채집,등산화,캠핑".to_string())
        .split(',').map(|s| s.trim().to_string()).collect();

    let ad_cache: AdCache = Arc::new(RwLock::new(vec![]));

    // reqwest::Client는 앱 전체에서 1개를 공유 (Connection pool/DNS/TLS 세션 재사용 → 메모리 안정)
    let http_client = reqwest::Client::new();

    if !access_key.is_empty() {
        let ads = coupang::fetch_ads(&http_client, &access_key, &secret_key, &keywords).await;
        println!("초기 광고 {}개 로드 완료", ads.len());
        *ad_cache.write().await = ads;
    }

    let cache_clone = ad_cache.clone();
    let http_bg = http_client.clone(); // Client 내부가 Arc라 clone은 cheap
    let ak = access_key.clone();
    let sk = secret_key.clone();
    let kw = keywords.clone();
    tokio::spawn(async move {
        if ak.is_empty() { return; }
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
            let ads = coupang::fetch_ads(&http_bg, &ak, &sk, &kw).await;
            println!("광고 갱신 완료: {}개", ads.len());
            *cache_clone.write().await = ads;
        }
    });

    let ad_cache_data = web::Data::new(ad_cache);
    let http_data = web::Data::new(http_client);
    let app_origin = env::var("APP_BASE_URL").unwrap_or_else(|_| "http://localhost:8888".to_string());

    HttpServer::new(move || {
        let cors = Cors::default()
            .allowed_origin(&app_origin)
            .allow_any_method()
            .allow_any_header()
            .supports_credentials()
            .max_age(3600);

        App::new()
            .wrap(Compress::default())
            .wrap(cors)
            .app_data(web::Data::new(pool.clone()))
            .app_data(ad_cache_data.clone())
            .app_data(http_data.clone())
            // 폴리곤
            .service(get_nearby_data)
            .service(get_nearest_polygon)
            // 광고
            .service(get_ads)
            // 인증
            .service(auth::google_login)
            .service(auth::google_callback)
            .service(auth::naver_login)
            .service(auth::naver_callback)
            .service(auth::kakao_login)
            .service(auth::kakao_callback)
            .service(auth::get_me)
            .service(auth::logout)
            // 저장 위치
            .service(create_location)
            .service(get_locations)
            .service(delete_location)
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
