# 개발 문서

## 디렉토리 구조

```
chamnamu_renew/
├── api/                     # Rust 백엔드 (Actix-web)
│   ├── src/
│   │   ├── main.rs          # 라우터, 서버 부팅, 광고 캐시
│   │   ├── db.rs            # PostGIS 쿼리, 마이그레이션, DTO
│   │   ├── auth.rs          # OAuth(Google/Naver/Kakao) + JWT
│   │   └── coupang.rs       # 쿠팡 파트너스 광고 fetch
│   ├── Cargo.toml
│   └── Dockerfile
├── frontend/                # SvelteKit 프론트엔드
│   ├── src/
│   │   ├── routes/
│   │   │   ├── +page.svelte         # 메인 지도 페이지
│   │   │   ├── +layout.svelte
│   │   │   └── privacy/             # 개인정보처리방침
│   │   └── lib/
│   │       ├── KakaoMap.svelte      # 지도 + 폴리곤 + 마커 + 나침반
│   │       ├── CollectingForecast.svelte   # 채집 예보 패널
│   │       ├── auth.ts              # 로그인/저장위치 클라이언트
│   │       └── assets/
│   ├── package.json
│   └── Dockerfile
├── nginx/                   # 정적 호스팅 + API 프록시
├── docker-compose.yml
└── docs/
    └── DEVELOPMENT.md
```

## 환경 변수 (.env)

| 키 | 설명 |
|---|---|
| `DB_USER`, `DB_NAME`, `DB_PASSWORD` | PostgreSQL 자격 |
| `DATABASE_URL` | `postgres://user:pwd@db:5432/dbname` |
| `JWT_SECRET` | JWT 서명 키 (랜덤 32+ 바이트 권장) |
| `APP_BASE_URL` | OAuth redirect용 절대 URL (예: `https://cnm.zam.kr`) |
| `GOOGLE_CLIENT_ID/SECRET` | Google OAuth |
| `NAVER_CLIENT_ID/SECRET` | 네이버 OAuth |
| `KAKAO_CLIENT_ID/SECRET` | 카카오 OAuth |
| `COUPANG_ACCESS_KEY/SECRET_KEY` | 쿠팡 파트너스 |
| `COUPANG_KEYWORDS` | 광고 검색어 (콤마 구분) |

## API 엔드포인트

### 폴리곤
| 메서드 | 경로 | 설명 |
|---|---|---|
| GET | `/api/polygon/nearby?lng=&lat=&distance=` | 반경 내 참나무 폴리곤(GeoJSON) |

### 광고
| 메서드 | 경로 | 설명 |
|---|---|---|
| GET | `/api/ads` | 쿠팡 광고 (메모리 캐시, 1시간 TTL) |

### 인증 (OAuth)
| 메서드 | 경로 | 설명 |
|---|---|---|
| GET | `/api/auth/{google\|naver\|kakao}` | OAuth 로그인 시작 |
| GET | `/api/auth/callback/{provider}` | OAuth 콜백 → JWT 발급 |
| GET | `/api/me` | 현재 로그인 사용자 정보 |

### 저장 위치 (JWT 필요)
| 메서드 | 경로 | 설명 |
|---|---|---|
| POST | `/api/locations` | `{ name, lat, lng, memo? }` |
| GET | `/api/locations` | 사용자의 저장된 위치 목록 |
| DELETE | `/api/locations/{id}` | 위치 삭제 |

## DB 스키마

```sql
-- users (OAuth 사용자)
id SERIAL PRIMARY KEY,
provider VARCHAR(20),       -- 'google' | 'naver' | 'kakao'
provider_id VARCHAR(100),
nickname VARCHAR(100),
created_at TIMESTAMPTZ,
UNIQUE(provider, provider_id)

-- saved_locations (채집 포인트)
id SERIAL PRIMARY KEY,
user_id INTEGER REFERENCES users(id) ON DELETE CASCADE,
name VARCHAR(200),
lat, lng DOUBLE PRECISION,
memo TEXT,
created_at TIMESTAMPTZ

-- 임상도 테이블 (외부 임포트, dump.sql)
-- 참나무 군락 MultiPolygon 데이터, EPSG:5179 → 4326 변환 사용
```

폴리곤 쿼리는 `ST_AsGeoJSON(ST_Transform(wkb_geometry, 4326), 6)`으로 6자리 좌표 + GeoJSON 응답 → gzip 압축으로 페이로드 최소화.

## 개발 환경 실행

### 도커 (권장)
```bash
docker compose up -d
docker compose logs -f api
```

### 개별 실행 (로컬 개발용)
```bash
# 백엔드
cd api
cargo run

# 프론트엔드
cd frontend
pnpm install
pnpm dev   # http://localhost:5173
```

## 핵심 모듈 설명

### `frontend/src/lib/KakaoMap.svelte`
- 지도 초기화, 카카오맵 SDK 로드
- `/api/polygon/nearby` 호출 → GeoJSON 파싱 → 카카오맵 폴리곤
- 디바이스 방향(`deviceorientation`) + GPS(`watchPosition`)로 나침반/현재위치
- 줌 레벨 변경 시 폴리곤 재조회 (10초 타임아웃, 실패시 재시도 가능)

### `frontend/src/lib/CollectingForecast.svelte`
채집 예보 패널. 주요 책임:
- **달 위상 계산**: Meeus 천문 알고리즘 (mean elongation D + 교란항 보정 6개)
  - 평균 신월 오차 ~2시간 (d5.co.kr 기준 일치)
  - 위상 분류: 상현/보름/하현/삭은 1일 좁은 띠, 사이는 6.4일 긴 구간
- **날씨**: Open-Meteo `forecast?hourly=temperature_2m,precipitation,windspeed_10m,weathercode` 24시간
- **위치명**: Nominatim 역지오코딩 (`/reverse?lat=&lon=&zoom=10`)
- **점수**: 기온 22~27℃, 강수 0mm, 풍속 ≤3m/s, 달빛 약함이 만점 5점
- **상호작용**: 드래그-다운/맵-탭/X 버튼으로 닫기, 달력 무한 스크롤(30일씩 + 365일까지)
- **로딩 정책**: 패널 첫 오픈 시에만 fetch, 좌표 변경 + 패널 열린 상태일 때 재조회

### `api/src/db.rs::get_nearby_polygon_data`
```rust
SELECT ogc_fid as id,
       ST_AsGeoJSON(ST_Transform(wkb_geometry, 4326), 6) AS geometry
FROM <임상도테이블>
WHERE ST_DWithin(
    ST_Transform(wkb_geometry, 4326)::geography,
    ST_SetSRID(ST_MakePoint($1, $2), 4326)::geography,
    $3  -- meters
)
```

### `api/src/auth.rs`
- 각 OAuth provider별 authorize/callback 핸들러
- callback에서 access_token → 사용자 정보 → `users` upsert → JWT 발급
- JWT는 `Authorization: Bearer <token>` 헤더로 전달, `extract_token()` 함수로 검증

## 빌드/배포

### 프론트엔드
```bash
cd frontend
pnpm build   # @sveltejs/adapter-static 사용
# 결과물: build/ → nginx에서 정적 호스팅
```

### 백엔드 (Docker 멀티스테이지)
- 1단계: rust:1.88-slim에서 `cargo build --release`
- 2단계: debian:bookworm-slim 런타임 이미지
- 의존성 캐싱: Cargo.toml/lock만 먼저 복사 → 더미 main.rs로 빌드 → 실제 소스 복사

### 배포 시 주의사항
- `.env` 노출 금지 (.gitignore 등록됨)
- DB 백업: `pg_dump`로 정기 스냅샷 권장
- nginx에 HTTPS(LetsEncrypt) 설정 필수 (OAuth redirect 요건)
- 카카오맵 API 키는 도메인 화이트리스트 등록

## 트러블슈팅

| 증상 | 원인/해결 |
|---|---|
| Docker build `cannot copy to non-directory: node_modules/...` | `.dockerignore`에 `node_modules` 누락 → 추가 |
| 폴리곤 안 그려짐 | API 응답이 빈 배열 (DB에 임상도 미임포트) 또는 거리 너무 작음 |
| 로그인 후 401 | JWT 만료 / `JWT_SECRET` 변경됨 / 쿠키-Authorization 헤더 미스매치 |
| 4월 19일에 삭으로 표시 | 구버전 코드 (90fd4a0 이전) → 새 Meeus 공식으로 업데이트 |
| 채집예보가 앱 시작 시 자동 로딩됨 | 90fd4a0에서 수정됨 (`show` 조건 추가) |

## 관련 커밋 / 히스토리

- `90fd4a0` 달 위상 정확도 개선 + 채집예보 UX 개선
- `92405eb` 나침반 GPS 연동 + 한달 달력
- `b8ceddc` 채집예보 시간 범위를 현재시각부터 24시간으로 변경
- `git log --oneline` 로 전체 히스토리 확인
