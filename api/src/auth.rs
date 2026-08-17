use actix_web::{get, post, web, HttpRequest, HttpResponse};
use actix_web::cookie::{time::Duration as CookieDuration, Cookie, SameSite};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::OnceLock;

// ── JWT ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: i32,       // user_id
    pub nickname: String,
    pub exp: usize,
}

// JWT 시크릿은 시작 시 한 번만 읽고 재사용 (env::var + String alloc 반복 제거)
static JWT_SECRET: OnceLock<Vec<u8>> = OnceLock::new();

fn validate_jwt_secret(secret: &str) -> Result<(), &'static str> {
    if secret.trim() != secret {
        return Err("JWT_SECRET must not have leading or trailing whitespace");
    }
    if secret.as_bytes().len() < 32 {
        return Err("JWT_SECRET must be at least 32 bytes long");
    }
    Ok(())
}

fn jwt_secret() -> &'static [u8] {
    JWT_SECRET.get_or_init(|| {
        let secret = env::var("JWT_SECRET").expect("JWT_SECRET must be set");
        validate_jwt_secret(&secret).unwrap_or_else(|message| panic!("{message}"));
        secret.into_bytes()
    })
}

pub fn ensure_jwt_secret() {
    let _ = jwt_secret();
}

pub fn create_jwt(user_id: i32, nickname: &str) -> String {
    let exp = (chrono::Utc::now() + chrono::Duration::days(30)).timestamp() as usize;
    let claims = Claims { sub: user_id, nickname: nickname.to_string(), exp };
    encode(&Header::default(), &claims, &EncodingKey::from_secret(jwt_secret()))
        .unwrap_or_default()
}

pub fn verify_jwt(token: &str) -> Option<Claims> {
    decode::<Claims>(token, &DecodingKey::from_secret(jwt_secret()), &Validation::new(Algorithm::HS256))
        .map(|d| d.claims)
        .ok()
}

// 로그인 상태는 httpOnly 쿠키(auth_token)로 유지함 - JS가 토큰 값을 직접 읽을 수 없어서
// XSS가 터져도 토큰 자체를 빼돌려 다른 곳에서 재사용하는 건 막을 수 있음.
// Authorization 헤더도 계속 지원(호환용) - 쿠키가 없으면 헤더를 확인함.
pub const AUTH_COOKIE_NAME: &str = "auth_token";

pub fn extract_token(req: &HttpRequest) -> Option<Claims> {
    let from_header = req.headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .and_then(|token| verify_jwt(token));
    if from_header.is_some() {
        return from_header;
    }
    req.cookie(AUTH_COOKIE_NAME).and_then(|c| verify_jwt(c.value()))
}

fn app_base_url() -> String {
    env::var("APP_BASE_URL").unwrap_or_else(|_| "http://localhost:8888".to_string())
}

fn build_auth_cookie(token: String) -> Cookie<'static> {
    Cookie::build(AUTH_COOKIE_NAME, token)
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(app_base_url().starts_with("https://"))
        .max_age(CookieDuration::days(30))
        .finish()
}

fn build_logout_cookie() -> Cookie<'static> {
    Cookie::build(AUTH_COOKIE_NAME, "")
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(app_base_url().starts_with("https://"))
        .max_age(CookieDuration::seconds(0))
        .finish()
}

fn redirect_to_frontend(token: String, provider: &str) -> HttpResponse {
    let url = format!("{}/", app_base_url());
    HttpResponse::Found()
        .append_header(("Location", url))
        .cookie(build_auth_cookie(token))
        .cookie(build_clear_state_cookie(provider))
        .finish()
}

fn redirect_error(provider: &str, msg: &str) -> HttpResponse {
    let url = format!("{}/?auth_error={}", app_base_url(), urlencoding::encode(msg));
    HttpResponse::Found()
        .append_header(("Location", url))
        .cookie(build_clear_state_cookie(provider))
        .finish()
}

fn redirect_invalid_state() -> HttpResponse {
    let url = format!(
        "{}/?auth_error={}",
        app_base_url(),
        urlencoding::encode("invalid state")
    );
    HttpResponse::Found()
        .append_header(("Location", url))
        .finish()
}

// ── OAuth 공통 ────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CallbackQuery {
    pub code: Option<String>,
    pub error: Option<String>,
    pub state: Option<String>,
}

// OAuth state는 로그인 요청을 시작한 브라우저가 맞는지 확인하는 CSRF 방지용 값.
// 로그인 리다이렉트 시 랜덤 값을 만들어 HttpOnly 쿠키에 심어두고, 콜백에서 쿼리로 돌아온
// state와 쿠키값이 일치하는지 확인함 - 일치하지 않으면 공격자가 자기 code로 만든 콜백
// URL을 피해자에게 클릭시켜 피해자를 공격자 계정으로 로그인시키는 로그인 CSRF가 가능해짐.
fn oauth_state_cookie_name(provider: &str) -> String {
    format!("oauth_state_{}", provider)
}

fn build_state_cookie(provider: &str, state: String) -> Cookie<'static> {
    Cookie::build(oauth_state_cookie_name(provider), state)
        .path("/api/auth")
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(app_base_url().starts_with("https://"))
        .max_age(CookieDuration::minutes(10))
        .finish()
}

fn build_clear_state_cookie(provider: &str) -> Cookie<'static> {
    Cookie::build(oauth_state_cookie_name(provider), "")
        .path("/api/auth")
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(app_base_url().starts_with("https://"))
        .max_age(CookieDuration::seconds(0))
        .finish()
}

fn validate_oauth_state(req: &HttpRequest, provider: &str, query_state: &Option<String>) -> bool {
    let cookie_value = req.cookie(&oauth_state_cookie_name(provider));
    match (cookie_value, query_state) {
        (Some(cookie), Some(query)) => !query.is_empty() && cookie.value() == query,
        _ => false,
    }
}

// ── Google ────────────────────────────────────────────────────────────────────

#[get("/api/auth/google")]
pub async fn google_login() -> HttpResponse {
    let client_id = env::var("GOOGLE_CLIENT_ID").unwrap_or_default();
    if client_id.is_empty() {
        return HttpResponse::ServiceUnavailable().body("Google OAuth not configured");
    }
    let redirect_uri = format!("{}/api/auth/callback/google", app_base_url());
    let state = uuid::Uuid::new_v4().to_string();
    let url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&redirect_uri={}&response_type=code&scope=openid%20email%20profile&access_type=offline&state={}",
        client_id, urlencoding::encode(&redirect_uri), state
    );
    HttpResponse::Found()
        .append_header(("Location", url))
        .cookie(build_state_cookie("google", state))
        .finish()
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
    req: HttpRequest,
    query: web::Query<CallbackQuery>,
    pool: web::Data<deadpool_postgres::Pool>,
    http: web::Data<Client>,
) -> HttpResponse {
    if !validate_oauth_state(&req, "google", &query.state) {
        return redirect_invalid_state();
    }
    if let Some(err) = &query.error {
        return redirect_error("google", err);
    }
    let code = match &query.code {
        Some(c) => c.clone(),
        None => return redirect_error("google", "code missing"),
    };

    let client_id = env::var("GOOGLE_CLIENT_ID").unwrap_or_default();
    let client_secret = env::var("GOOGLE_CLIENT_SECRET").unwrap_or_default();
    let redirect_uri = format!("{}/api/auth/callback/google", app_base_url());

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
            Err(e) => { eprintln!("[Google] token parse error: {}", e); return redirect_error("google", "token error"); }
        },
        Err(e) => { eprintln!("[Google] token request error: {}", e); return redirect_error("google", "token request failed"); }
    };

    // 사용자 정보
    let user_info: GoogleUserInfo = match http
        .get("https://www.googleapis.com/oauth2/v3/userinfo")
        .bearer_auth(&token_res.access_token)
        .send().await
    {
        Ok(r) => match r.json().await {
            Ok(u) => u,
            Err(e) => { eprintln!("[Google] userinfo parse error: {}", e); return redirect_error("google", "userinfo error"); }
        },
        Err(e) => { eprintln!("[Google] userinfo error: {}", e); return redirect_error("google", "userinfo failed"); }
    };

    let user_id = match crate::db::upsert_user(
        &pool, "google", &user_info.sub, &user_info.name,
    ).await {
        Ok(id) => id,
        Err(e) => { eprintln!("[Google] db error: {}", e); return redirect_error("google", "db error"); }
    };

    let token = create_jwt(user_id, &user_info.name);
    redirect_to_frontend(token, "google")
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
    HttpResponse::Found()
        .append_header(("Location", url))
        .cookie(build_state_cookie("naver", state))
        .finish()
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
    req: HttpRequest,
    query: web::Query<CallbackQuery>,
    pool: web::Data<deadpool_postgres::Pool>,
    http: web::Data<Client>,
) -> HttpResponse {
    if !validate_oauth_state(&req, "naver", &query.state) {
        return redirect_invalid_state();
    }
    if let Some(err) = &query.error {
        return redirect_error("naver", err);
    }
    let code = match &query.code {
        Some(c) => c.clone(),
        None => return redirect_error("naver", "code missing"),
    };

    let client_id = env::var("NAVER_CLIENT_ID").unwrap_or_default();
    let client_secret = env::var("NAVER_CLIENT_SECRET").unwrap_or_default();
    let redirect_uri = format!("{}/api/auth/callback/naver", app_base_url());

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
            Err(e) => { eprintln!("[Naver] token parse: {}", e); return redirect_error("naver", "token error"); }
        },
        Err(e) => { eprintln!("[Naver] token req: {}", e); return redirect_error("naver", "token request failed"); }
    };

    let user_res: NaverUserResponse = match http
        .get("https://openapi.naver.com/v1/nid/me")
        .bearer_auth(&token_res.access_token)
        .send().await
    {
        Ok(r) => match r.json().await {
            Ok(u) => u,
            Err(e) => { eprintln!("[Naver] userinfo parse: {}", e); return redirect_error("naver", "userinfo error"); }
        },
        Err(e) => { eprintln!("[Naver] userinfo req: {}", e); return redirect_error("naver", "userinfo failed"); }
    };

    let u = user_res.response;
    let nickname = u.nickname.unwrap_or_else(|| "네이버 사용자".to_string());

    let user_id = match crate::db::upsert_user(
        &pool, "naver", &u.id, &nickname,
    ).await {
        Ok(id) => id,
        Err(e) => { eprintln!("[Naver] db error: {}", e); return redirect_error("naver", "db error"); }
    };

    let token = create_jwt(user_id, &nickname);
    redirect_to_frontend(token, "naver")
}

// ── Kakao ─────────────────────────────────────────────────────────────────────

#[get("/api/auth/kakao")]
pub async fn kakao_login() -> HttpResponse {
    let client_id = env::var("KAKAO_CLIENT_ID").unwrap_or_default();
    if client_id.is_empty() {
        return HttpResponse::ServiceUnavailable().body("Kakao OAuth not configured");
    }
    let redirect_uri = format!("{}/api/auth/callback/kakao", app_base_url());
    let state = uuid::Uuid::new_v4().to_string();
    let url = format!(
        "https://kauth.kakao.com/oauth/authorize?client_id={}&redirect_uri={}&response_type=code&state={}",
        client_id, urlencoding::encode(&redirect_uri), state
    );
    HttpResponse::Found()
        .append_header(("Location", url))
        .cookie(build_state_cookie("kakao", state))
        .finish()
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
    req: HttpRequest,
    query: web::Query<CallbackQuery>,
    pool: web::Data<deadpool_postgres::Pool>,
    http: web::Data<Client>,
) -> HttpResponse {
    if !validate_oauth_state(&req, "kakao", &query.state) {
        return redirect_invalid_state();
    }
    if let Some(err) = &query.error {
        return redirect_error("kakao", err);
    }
    let code = match &query.code {
        Some(c) => c.clone(),
        None => return redirect_error("kakao", "code missing"),
    };

    let client_id = env::var("KAKAO_CLIENT_ID").unwrap_or_default();
    let client_secret = env::var("KAKAO_CLIENT_SECRET").unwrap_or_default();
    let redirect_uri = format!("{}/api/auth/callback/kakao", app_base_url());

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
            Err(e) => { eprintln!("[Kakao] token parse: {}", e); return redirect_error("kakao", "token error"); }
        },
        Err(e) => { eprintln!("[Kakao] token req: {}", e); return redirect_error("kakao", "token request failed"); }
    };

    let user_info: KakaoUserInfo = match http
        .get("https://kapi.kakao.com/v2/user/me")
        .bearer_auth(&token_res.access_token)
        .send().await
    {
        Ok(r) => match r.json().await {
            Ok(u) => u,
            Err(e) => { eprintln!("[Kakao] userinfo parse: {}", e); return redirect_error("kakao", "userinfo error"); }
        },
        Err(e) => { eprintln!("[Kakao] userinfo req: {}", e); return redirect_error("kakao", "userinfo failed"); }
    };

    let provider_id = user_info.id.to_string();
    let account = user_info.kakao_account.unwrap_or(KakaoAccount { profile: None });
    let profile = account.profile.unwrap_or(KakaoProfile { nickname: None });
    let nickname = profile.nickname.unwrap_or_else(|| "카카오 사용자".to_string());

    let user_id = match crate::db::upsert_user(
        &pool, "kakao", &provider_id, &nickname,
    ).await {
        Ok(id) => id,
        Err(e) => { eprintln!("[Kakao] db error: {}", e); return redirect_error("kakao", "db error"); }
    };

    let token = create_jwt(user_id, &nickname);
    redirect_to_frontend(token, "kakao")
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

#[post("/api/auth/logout")]
pub async fn logout() -> HttpResponse {
    HttpResponse::Ok().cookie(build_logout_cookie()).finish()
}

#[cfg(test)]
mod tests {
    use super::validate_jwt_secret;

    #[test]
    fn jwt_secret_requires_at_least_32_bytes() {
        assert!(validate_jwt_secret("").is_err());
        assert!(validate_jwt_secret("short-secret").is_err());
        assert!(validate_jwt_secret("12345678901234567890123456789012").is_ok());
    }

    #[test]
    fn jwt_secret_rejects_surrounding_whitespace() {
        assert!(validate_jwt_secret(" 12345678901234567890123456789012").is_err());
        assert!(validate_jwt_secret("12345678901234567890123456789012 ").is_err());
    }
}
