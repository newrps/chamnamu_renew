use actix_web::{get, web, HttpRequest, HttpResponse};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;

// ── JWT ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: i32,       // user_id
    pub nickname: String,
    pub exp: usize,
}

pub fn create_jwt(user_id: i32, nickname: &str) -> String {
    let secret = env::var("JWT_SECRET").unwrap_or_else(|_| "chamnamu_secret_key_2024".to_string());
    let exp = (chrono::Utc::now() + chrono::Duration::days(30)).timestamp() as usize;
    let claims = Claims { sub: user_id, nickname: nickname.to_string(), exp };
    encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes()))
        .unwrap_or_default()
}

pub fn verify_jwt(token: &str) -> Option<Claims> {
    let secret = env::var("JWT_SECRET").unwrap_or_else(|_| "chamnamu_secret_key_2024".to_string());
    decode::<Claims>(token, &DecodingKey::from_secret(secret.as_bytes()), &Validation::new(Algorithm::HS256))
        .map(|d| d.claims)
        .ok()
}

pub fn extract_token(req: &HttpRequest) -> Option<Claims> {
    req.headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .and_then(|token| verify_jwt(token))
}

fn app_base_url() -> String {
    env::var("APP_BASE_URL").unwrap_or_else(|_| "http://localhost:8888".to_string())
}

fn redirect_to_frontend(token: &str) -> HttpResponse {
    let url = format!("{}/?token={}", app_base_url(), token);
    HttpResponse::Found().append_header(("Location", url)).finish()
}

fn redirect_error(msg: &str) -> HttpResponse {
    let url = format!("{}/?auth_error={}", app_base_url(), urlencoding::encode(msg));
    HttpResponse::Found().append_header(("Location", url)).finish()
}

// ── OAuth 공통 ────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CallbackQuery {
    pub code: Option<String>,
    pub error: Option<String>,
}

// ── Google ────────────────────────────────────────────────────────────────────

#[get("/api/auth/google")]
pub async fn google_login() -> HttpResponse {
    let client_id = env::var("GOOGLE_CLIENT_ID").unwrap_or_default();
    if client_id.is_empty() {
        return HttpResponse::ServiceUnavailable().body("Google OAuth not configured");
    }
    let redirect_uri = format!("{}/api/auth/callback/google", app_base_url());
    let url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&redirect_uri={}&response_type=code&scope=openid%20email%20profile&access_type=offline",
        client_id, urlencoding::encode(&redirect_uri)
    );
    HttpResponse::Found().append_header(("Location", url)).finish()
}

#[derive(Deserialize)]
struct GoogleTokenRes {
    access_token: String,
}

#[derive(Deserialize)]
struct GoogleUserInfo {
    name: String,
    sub: String,
}

#[get("/api/auth/callback/google")]
pub async fn google_callback(
    query: web::Query<CallbackQuery>,
    pool: web::Data<deadpool_postgres::Pool>,
) -> HttpResponse {
    if let Some(err) = &query.error {
        return redirect_error(err);
    }
    let code = match &query.code {
        Some(c) => c.clone(),
        None => return redirect_error("code missing"),
    };

    let client_id = env::var("GOOGLE_CLIENT_ID").unwrap_or_default();
    let client_secret = env::var("GOOGLE_CLIENT_SECRET").unwrap_or_default();
    let redirect_uri = format!("{}/api/auth/callback/google", app_base_url());

    let http = Client::new();

    // 토큰 교환
    let token_res: GoogleTokenRes = match http.post("https://oauth2.googleapis.com/token")
        .form(&[
            ("code", code.as_str()),
            ("client_id", &client_id),
            ("client_secret", &client_secret),
            ("redirect_uri", &redirect_uri),
            ("grant_type", "authorization_code"),
        ])
        .send().await
    {
        Ok(r) => match r.json().await {
            Ok(t) => t,
            Err(e) => { eprintln!("[Google] token parse error: {}", e); return redirect_error("token error"); }
        },
        Err(e) => { eprintln!("[Google] token request error: {}", e); return redirect_error("token request failed"); }
    };

    // 사용자 정보
    let user_info: GoogleUserInfo = match http
        .get("https://www.googleapis.com/oauth2/v3/userinfo")
        .bearer_auth(&token_res.access_token)
        .send().await
    {
        Ok(r) => match r.json().await {
            Ok(u) => u,
            Err(e) => { eprintln!("[Google] userinfo parse error: {}", e); return redirect_error("userinfo error"); }
        },
        Err(e) => { eprintln!("[Google] userinfo error: {}", e); return redirect_error("userinfo failed"); }
    };

    let user_id = match crate::db::upsert_user(
        &pool, "google", &user_info.sub, &user_info.name,
    ).await {
        Ok(id) => id,
        Err(e) => { eprintln!("[Google] db error: {}", e); return redirect_error("db error"); }
    };

    let token = create_jwt(user_id, &user_info.name);
    redirect_to_frontend(&token)
}

// ── Naver ─────────────────────────────────────────────────────────────────────

#[get("/api/auth/naver")]
pub async fn naver_login() -> HttpResponse {
    let client_id = env::var("NAVER_CLIENT_ID").unwrap_or_default();
    if client_id.is_empty() {
        return HttpResponse::ServiceUnavailable().body("Naver OAuth not configured");
    }
    let redirect_uri = format!("{}/api/auth/callback/naver", app_base_url());
    let state = uuid::Uuid::new_v4().to_string();
    let url = format!(
        "https://nid.naver.com/oauth2.0/authorize?client_id={}&redirect_uri={}&response_type=code&state={}",
        client_id, urlencoding::encode(&redirect_uri), state
    );
    HttpResponse::Found().append_header(("Location", url)).finish()
}

#[derive(Deserialize)]
struct NaverTokenRes {
    access_token: String,
}

#[derive(Deserialize)]
struct NaverUserResponse {
    response: NaverUserInfo,
}

#[derive(Deserialize)]
struct NaverUserInfo {
    id: String,
    nickname: Option<String>,
}

#[get("/api/auth/callback/naver")]
pub async fn naver_callback(
    query: web::Query<CallbackQuery>,
    pool: web::Data<deadpool_postgres::Pool>,
) -> HttpResponse {
    if let Some(err) = &query.error {
        return redirect_error(err);
    }
    let code = match &query.code {
        Some(c) => c.clone(),
        None => return redirect_error("code missing"),
    };

    let client_id = env::var("NAVER_CLIENT_ID").unwrap_or_default();
    let client_secret = env::var("NAVER_CLIENT_SECRET").unwrap_or_default();
    let redirect_uri = format!("{}/api/auth/callback/naver", app_base_url());

    let http = Client::new();

    let token_res: NaverTokenRes = match http
        .post("https://nid.naver.com/oauth2.0/token")
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", &client_id),
            ("client_secret", &client_secret),
            ("code", &code),
            ("redirect_uri", &redirect_uri),
        ])
        .send().await
    {
        Ok(r) => match r.json().await {
            Ok(t) => t,
            Err(e) => { eprintln!("[Naver] token parse: {}", e); return redirect_error("token error"); }
        },
        Err(e) => { eprintln!("[Naver] token req: {}", e); return redirect_error("token request failed"); }
    };

    let user_res: NaverUserResponse = match http
        .get("https://openapi.naver.com/v1/nid/me")
        .bearer_auth(&token_res.access_token)
        .send().await
    {
        Ok(r) => match r.json().await {
            Ok(u) => u,
            Err(e) => { eprintln!("[Naver] userinfo parse: {}", e); return redirect_error("userinfo error"); }
        },
        Err(e) => { eprintln!("[Naver] userinfo req: {}", e); return redirect_error("userinfo failed"); }
    };

    let u = user_res.response;
    let nickname = u.nickname.unwrap_or_else(|| "네이버 사용자".to_string());

    let user_id = match crate::db::upsert_user(
        &pool, "naver", &u.id, &nickname,
    ).await {
        Ok(id) => id,
        Err(e) => { eprintln!("[Naver] db error: {}", e); return redirect_error("db error"); }
    };

    let token = create_jwt(user_id, &nickname);
    redirect_to_frontend(&token)
}

// ── Kakao ─────────────────────────────────────────────────────────────────────

#[get("/api/auth/kakao")]
pub async fn kakao_login() -> HttpResponse {
    let client_id = env::var("KAKAO_CLIENT_ID").unwrap_or_default();
    if client_id.is_empty() {
        return HttpResponse::ServiceUnavailable().body("Kakao OAuth not configured");
    }
    let redirect_uri = format!("{}/api/auth/callback/kakao", app_base_url());
    let url = format!(
        "https://kauth.kakao.com/oauth/authorize?client_id={}&redirect_uri={}&response_type=code",
        client_id, urlencoding::encode(&redirect_uri)
    );
    HttpResponse::Found().append_header(("Location", url)).finish()
}

#[derive(Deserialize)]
struct KakaoTokenRes {
    access_token: String,
}

#[derive(Deserialize)]
struct KakaoUserInfo {
    id: i64,
    kakao_account: Option<KakaoAccount>,
}

#[derive(Deserialize)]
struct KakaoAccount {
    profile: Option<KakaoProfile>,
}

#[derive(Deserialize)]
struct KakaoProfile {
    nickname: Option<String>,
}

#[get("/api/auth/callback/kakao")]
pub async fn kakao_callback(
    query: web::Query<CallbackQuery>,
    pool: web::Data<deadpool_postgres::Pool>,
) -> HttpResponse {
    if let Some(err) = &query.error {
        return redirect_error(err);
    }
    let code = match &query.code {
        Some(c) => c.clone(),
        None => return redirect_error("code missing"),
    };

    let client_id = env::var("KAKAO_CLIENT_ID").unwrap_or_default();
    let client_secret = env::var("KAKAO_CLIENT_SECRET").unwrap_or_default();
    let redirect_uri = format!("{}/api/auth/callback/kakao", app_base_url());

    let http = Client::new();

    let mut params = vec![
        ("grant_type", "authorization_code"),
        ("client_id", &client_id),
        ("redirect_uri", &redirect_uri),
        ("code", &code),
    ];
    if !client_secret.is_empty() {
        params.push(("client_secret", &client_secret));
    }

    let token_res: KakaoTokenRes = match http
        .post("https://kauth.kakao.com/oauth/token")
        .form(&params)
        .send().await
    {
        Ok(r) => match r.json().await {
            Ok(t) => t,
            Err(e) => { eprintln!("[Kakao] token parse: {}", e); return redirect_error("token error"); }
        },
        Err(e) => { eprintln!("[Kakao] token req: {}", e); return redirect_error("token request failed"); }
    };

    let user_info: KakaoUserInfo = match http
        .get("https://kapi.kakao.com/v2/user/me")
        .bearer_auth(&token_res.access_token)
        .send().await
    {
        Ok(r) => match r.json().await {
            Ok(u) => u,
            Err(e) => { eprintln!("[Kakao] userinfo parse: {}", e); return redirect_error("userinfo error"); }
        },
        Err(e) => { eprintln!("[Kakao] userinfo req: {}", e); return redirect_error("userinfo failed"); }
    };

    let provider_id = user_info.id.to_string();
    let account = user_info.kakao_account.unwrap_or(KakaoAccount { profile: None });
    let profile = account.profile.unwrap_or(KakaoProfile { nickname: None });
    let nickname = profile.nickname.unwrap_or_else(|| "카카오 사용자".to_string());

    let user_id = match crate::db::upsert_user(
        &pool, "kakao", &provider_id, &nickname,
    ).await {
        Ok(id) => id,
        Err(e) => { eprintln!("[Kakao] db error: {}", e); return redirect_error("db error"); }
    };

    let token = create_jwt(user_id, &nickname);
    redirect_to_frontend(&token)
}

// ── /api/me ───────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct MeResponse {
    pub id: i32,
    pub nickname: String,
}

#[get("/api/me")]
pub async fn get_me(req: HttpRequest) -> HttpResponse {
    match extract_token(&req) {
        Some(claims) => HttpResponse::Ok().json(MeResponse {
            id: claims.sub,
            nickname: claims.nickname,
        }),
        None => HttpResponse::Unauthorized().finish(),
    }
}
