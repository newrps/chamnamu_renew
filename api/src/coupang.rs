use chrono::Utc;
use hmac::{Hmac, Mac};
use reqwest::Client;
use serde::Serialize;
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

const BG_COLORS: &[&str] = &[
    "linear-gradient(135deg, #ff6b6b, #ee5a24)",
    "linear-gradient(135deg, #4ecdc4, #44a08d)",
    "linear-gradient(135deg, #667eea, #764ba2)",
    "linear-gradient(135deg, #f093fb, #f5576c)",
    "linear-gradient(135deg, #4facfe, #00f2fe)",
];

#[derive(Serialize, Clone, Debug)]
pub struct AdItem {
    #[serde(rename = "type")]
    pub ad_type: String,
    pub url: String,
    pub content: String,
    #[serde(rename = "backgroundColor")]
    pub background_color: String,
}

fn make_signature(secret_key: &str, path: &str, query: &str, datetime: &str) -> String {
    // 쿠팡 파트너스 공식 서명 형식:
    // datetime + "GET" + path + query  (? 없이 path 바로 뒤에 query 붙임)
    // 참고: Python 공식 예제: message = datetimeGMT + method + path + query[0]
    let message = format!("{}GET{}{}", datetime, path, query);
    println!("[쿠팡 서명] message={}", message);
    let mut mac = HmacSha256::new_from_slice(secret_key.as_bytes())
        .expect("HMAC init failed");
    mac.update(message.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

fn format_price(price: i64) -> String {
    let s = price.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

pub async fn fetch_ads(
    access_key: &str,
    secret_key: &str,
    keywords: &[String],
) -> Vec<AdItem> {
    let client = Client::new();
    let mut all_ads = Vec::new();
    let mut color_idx = 0;

    for keyword in keywords {
        let encoded_keyword = urlencoding::encode(keyword);

        // path와 query를 분리 (서명할 때 ?를 포함하지 않음)
        let path = "/v2/providers/affiliate_open_api/apis/openapi/products/search";
        let query = format!("keyword={}&limit=5", encoded_keyword);

        let datetime = Utc::now().format("%y%m%dT%H%M%SZ").to_string();
        let signature = make_signature(secret_key, path, &query, &datetime);

        let authorization = format!(
            "CEA algorithm=HmacSHA256, access-key={}, signed-date={}, signature={}",
            access_key, datetime, signature
        );

        // 실제 요청 URL: ?를 포함해서 구성
        let url = format!("https://api-gateway.coupang.com{}?{}", path, query);

        println!("[쿠팡] keyword={} datetime={}", keyword, datetime);
        println!("[쿠팡] url={}", url);

        let result = client
            .get(&url)
            .header("Authorization", &authorization)
            .header("Content-Type", "application/json")
            .send()
            .await;

        match result {
            Ok(response) => {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                let preview: String = body.chars().take(200).collect();
                println!("[쿠팡 응답] status={} body={}", status, preview);

                if let Ok(data) = serde_json::from_str::<serde_json::Value>(&body) {
                    if let Some(products) = data["data"]["productData"].as_array() {
                        for product in products {
                            let name = product["productName"].as_str().unwrap_or("").to_string();
                            let url = product["productUrl"].as_str().unwrap_or("").to_string();
                            let price = product["productPrice"].as_i64().unwrap_or(0);

                            if name.is_empty() || url.is_empty() {
                                continue;
                            }

                            all_ads.push(AdItem {
                                ad_type: "text".to_string(),
                                url,
                                content: format!("{} - {}원", name, format_price(price)),
                                background_color: BG_COLORS[color_idx % BG_COLORS.len()].to_string(),
                            });
                            color_idx += 1;
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("[쿠팡 오류] keyword={}: {}", keyword, e);
            }
        }
    }

    all_ads
}
