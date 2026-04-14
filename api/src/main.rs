use actix_web::{delete, get, post, web, App, HttpRequest, HttpResponse, HttpServer, Result, error::ErrorBadRequest};
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
    let lng: f64 = query.get("lng").expect("Missing lng").parse().expect("Invalid lng");
    let lat: f64 = query.get("lat").expect("Missing lat").parse().expect("Invalid lat");
    let distance_str = query.get("distance").ok_or_else(|| ErrorBadRequest("Missing distance"))?;
    let distance = distance_str.parse::<f64>().map_err(|_| ErrorBadRequest("Invalid distance"))?;
    let list = db::get_nearby_polygon_data(pool, lng, lat, distance).await?;
    Ok(HttpResponse::Ok().json(list))
}

// ── 광고 ──────────────────────────────────────────────────────────────────────

#[get("/api/ads")]
async fn get_ads(cache: web::Data<AdCache>) -> HttpResponse {
    let ads = cache.read().await;
    HttpResponse::Ok().json(ads.clone())
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

    if !access_key.is_empty() {
        let ads = coupang::fetch_ads(&access_key, &secret_key, &keywords).await;
        println!("초기 광고 {}개 로드 완료", ads.len());
        *ad_cache.write().await = ads;
    }

    let cache_clone = ad_cache.clone();
    let ak = access_key.clone();
    let sk = secret_key.clone();
    let kw = keywords.clone();
    tokio::spawn(async move {
        if ak.is_empty() { return; }
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
            let ads = coupang::fetch_ads(&ak, &sk, &kw).await;
            println!("광고 갱신 완료: {}개", ads.len());
            *cache_clone.write().await = ads;
        }
    });

    let ad_cache_data = web::Data::new(ad_cache);

    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);

        App::new()
            .wrap(cors)
            .app_data(web::Data::new(pool.clone()))
            .app_data(ad_cache_data.clone())
            // 폴리곤
            .service(get_nearby_data)
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
            // 저장 위치
            .service(create_location)
            .service(get_locations)
            .service(delete_location)
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
