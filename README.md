# 깜장 참나무 지도 (Chamnamu Map)

곤충 채집을 위한 참나무 분포 지도와 채집 적합성 예보를 제공하는 웹 앱입니다.

## 주요 기능

- **참나무 분포 지도**: 산림청 임상도 기반 참나무 군락(Polygon) 시각화 (카카오맵)
- **채집 예보**: 시간별 날씨(기온/강수/풍속) + 달 위상을 종합한 24시간 채집 적합성 점수
- **달 달력**: 30일 ~ 1년 달 위상 일자별 표시 (Meeus 천문 알고리즘)
- **나침반/현재 위치**: 디바이스 방향 + GPS 연동
- **장소 저장**: 채집 포인트 메모와 함께 저장 (소셜 로그인 필요)
- **PWA**: 홈 화면 추가, 오프라인 지원 가능

## 기술 스택

| 계층 | 기술 |
|---|---|
| Frontend | SvelteKit 2 · Svelte 5 · TypeScript · Vite |
| 지도 | Kakao Maps SDK v2 |
| Backend | Rust · Actix-web 4 · Tokio |
| DB | PostgreSQL 17 + PostGIS 3.5 |
| 외부 API | Open-Meteo(날씨) · Nominatim(역지오코딩) · 쿠팡 파트너스 |
| 인증 | Google · 네이버 · 카카오 OAuth + JWT |
| 인프라 | Docker Compose · nginx |

## 빠른 시작

`.env` 파일 생성 후:

```bash
docker compose up -d
```

- 프론트엔드: http://localhost:8888
- API: 컨테이너 내부 8080
- DB: postgis/postgis:17-3.5

## 라이선스 / 데이터 출처

- 임상도: 산림청 산림빅데이터플랫폼
- 지도: ⓒ Kakao
- 날씨: Open-Meteo (CC BY 4.0)
- 주소: OpenStreetMap / Nominatim

## 문의

issues 또는 [개발 문서](./docs/DEVELOPMENT.md) 참고
