# 폴리곤 단순화 / 렌더링 성능 작업 (2026-08-19)

`/api/polygon/nearby` 응답이 너무 커서(폴리곤 점 개수 많음) 지도 렌더링이 느리고, 밀집 지역에서
일부 폴리곤이 누락되거나 타임아웃 나던 문제를 해결한 작업 기록. 관련 커밋(최신순):

```
9fa166c perf: 폴리곤 렌더링 최적화 - mousemove 리스너 제거, 화면 보이는 영역 우선 렌더링, 프레임 분할 생성
180ffd0 fix: 확대해도 예전 넓은 범위(단순화됨) 쿼리 캐시를 재사용해 세밀해지지 않던 버그 수정
02014a9 refactor: 줌레벨이 아닌 뷰포트 내 실제 폴리곤 개수 기준으로 단순화 정도 결정
2a3d3b4 fix: ST_Simplify preserveCollapsed 적용 + geometry NULL 방어 (502 유발 패닉 수정)
1a7f978 fix: ST_SimplifyPreserveTopology -> ST_Simplify로 변경 (20배 느려서 프론트 타임아웃 유발)
db44828 tune: 레벨4까지 원본 유지, 5레벨 5m/6레벨 10m/7레벨 20m로 단순화 구간 조정
6cf1df5 fix: 레벨별 폴리곤 조회 LIMIT 상향 + ORDER BY 추가 (밀집지역 누락 방지)
c5d1b65 perf: 카카오맵 줌레벨별 폴리곤 단순화(ST_SimplifyPreserveTopology) 적용
```

## 현재 아키텍처 (최종 상태)

### 1. 서버: 개수 기반 LOD (`api/src/db.rs`)

줌레벨이 아니라 **해당 bbox 안에 실제로 걸리는 폴리곤 개수**로 단순화 정도를 정한다.
줌레벨은 화면 크기에 따라 실제 커버 면적이 달라서(모바일은 화면이 작아 같은 레벨이라도
훨씬 좁은 면적만 요청함) 밀집도의 대리 지표로 부정확했기 때문.

```rust
fn lod_for_count(count: i64) -> (f64, i64) {
    match count {
        i64::MIN..=800 => (0.0, 800),        // 원본 그대로
        801..=3_000 => (3.0, 3_000),
        3_001..=8_000 => (8.0, 8_000),
        8_001..=20_000 => (15.0, 20_000),
        _ => (25.0, 40_000),                  // tolerance(m), row limit
    }
}
```

처리 순서 (`get_polygons_in_bbox`):
1. `SELECT count(*) ... WHERE wkb_geometry && bbox` 로 실제 개수 먼저 파악 (빠름, 인덱스 스캔)
2. 개수로 tolerance/row_limit 결정
3. `ST_Simplify(wkb_geometry, tolerance, true) → ST_Transform → ST_AsGeoJSON`
4. `ORDER BY ogc_fid LIMIT row_limit` — 순서 고정으로 같은 bbox는 항상 같은 부분집합 반환(깜빡임 방지)
5. 응답 헤더 `X-Simplify-Tolerance: <실제 적용값>` 으로 프론트에 알려줌 (같은 오리진 프록시라 CORS 이슈 없음)

### 2. 프론트: 캐시 키와 렌더링 (`frontend/src/lib/KakaoMap.svelte`)

- `drawnPolygons` 캐시 키: `${id}:${toleranceBucket}` — 같은 폴리곤이라도 상세도가 다르면
  별도 항목으로 캐싱해서, 다른 뷰에서 받은 뭉개진 버전이 섞이지 않게 함
- `polygonQueryCache`(bbox 단위 캐시)는 **`level`이 같을 때만** 재사용 — 패닝(같은 줌)은 캐시 재사용,
  줌이 바뀌면 무조건 새로 요청. (→ "확대해도 안 세밀해짐" 버그의 원인이자 수정 지점, 아래 이슈 참고)
- 화면에 실제 보이는 영역(`map.getBounds()`)부터 우선 그리고, 패딩 여유분은 뒤로 미룸
- 폴리곤 생성을 400개씩 `requestAnimationFrame`으로 나눠서 처리 (한 번에 수만 개 동기 생성 시
  메인스레드가 멈추는 것 방지). 그리는 도중 새 요청이 들어오면 `signal.aborted` 체크로 즉시 중단
- 폴리곤당 이벤트 리스너 4개(`mouseover`/`mousemove`/`mouseout`/`click`) → `mousemove` 제거해 3개로
  (툴팁이 마우스를 계속 따라다니진 않고 처음 hover한 위치에 고정됨)

## 시행착오 기록

1. **처음엔 줌레벨 기준으로 단순화** (`c5d1b65`) — 레벨→tolerance 매핑 테이블로 시작.
2. **레벨 7에서 밀집 지역 폴리곤이 통째로 안 보임** — 원인은 원래부터 있던 `LIMIT 10000` +
   `ORDER BY` 없음. bbox 안에 3만 개 있어도 앞 1만 개만(그것도 매번 다른 부분집합) 반환되고 있었음.
   → LIMIT을 레벨별로 상향 + `ORDER BY ogc_fid` 추가 (`6cf1df5`).
3. **`ST_SimplifyPreserveTopology`가 넓은 뷰포트(수만 건)에서 12초+ 걸려 프론트 10초 타임아웃 유발**
   → `ST_Simplify`로 교체, 21배 빨라짐(12.4s → 0.57s) (`1a7f978`).
4. **`ST_Simplify`는 작고 얇은 폴리곤을 통째로 지워버리며 NULL 반환 → Rust가 파싱 중 패닉 → 502**
   → `ST_Simplify(geom, tolerance, true)`(preserveCollapsed)로 해결 + Rust 쪽도 NULL이면
   서버가 죽지 않고 해당 폴리곤만 건너뛰도록 방어 코드 추가 (`2a3d3b4`).
5. **"모바일은 레벨7이어도 화면에 폴리곤이 얼마 없는데 그런 것치고 너무 뭉개진다"는 지적**
   → 줌레벨 기반을 완전히 폐기하고 **개수 기반**으로 전환 (`02014a9`). 이때부터 `level` 쿼리
   파라미터는 API에서 사라지고, 서버가 알아서 count를 세서 tolerance를 정하고 헤더로 알려줌.
6. **넓게 보다가(단순화됨) 확대해도 예전 뭉개진 도형이 계속 보임** — 쿼리 캐시가 "bounds가 포함되면
   무조건 재사용 가능"하다고 잘못 판단했던 게 원인 (레벨 체크를 개수 기반 전환 때 실수로 빼버림).
   → 쿼리 캐시 재사용 조건에 "같은 레벨일 때만" 다시 추가 (`180ffd0`).
7. **렌더링 자체(그리기) 성능 개선** — mousemove 리스너 제거, 화면에 보이는 부분 우선 그리기,
   `requestAnimationFrame` 프레임 분할 생성 (`9fa166c`).

## 알려진 이슈 / 한계 (미해결, 다음에 볼 것)

- **같은 줌레벨에서도 지역마다 단순화 정도가 다르게 보임** — 개수 기반 설계의 당연한 결과.
  밀집 지역(산)과 듬성듬성한 지역(도심 근처)을 같은 레벨로 오가면 detail이 바뀌는 게 정상 동작이라,
  "일관성 없어 보인다"는 체감이 있을 수 있음. 필요하면 구간 경계에 히스테리시스(한번 정해지면
  개수가 살짝 줄어도 당분간 유지)를 넣어 완화 가능 — 아직 미적용.
- **LOD 구간 경계값 근처에서 flapping 가능성** — bbox가 픽셀 단위로 완벽히 재현되지 않아서
  (드래그/줌 위치 미세한 차이), 개수가 800/3,000/8,000/20,000 근처를 오가면 같은 화면처럼
  보여도 tolerance가 다르게 나올 수 있음. 실측(±1% bbox 흔들기)으로는 재현 안 됐지만
  경계 근처 지역에선 발생 가능.
- **서버 row_limit 상한(4만)이 클라이언트 캐시 설정(`polygonCacheLimit`, PC 기준 1.8만~2.4만)보다 큼**
  — `evictUnusedPolygons()`는 "화면에 안 보이는" 것만 정리하지 "지금 보이는" 건 아무리 많아도
  안 지우므로, 밀집 지역에서 4만 개가 한 번에 활성 렌더링될 수 있음. row_limit을 낮추거나
  클라이언트 캐시 한도를 올려서 맞출지는 아직 결정 안 함.
- **폴리곤 하나 = `kakao.maps.Polygon` 객체 하나 + 리스너 3개** 구조라, 개수가 수만 개면 구조적으로
  무거움. 근본적으로 가벼워지려면 `CustomOverlay` + `<canvas>` 직접 렌더링(폴리곤 전체를 캔버스에
  직접 그리고 hover/click도 point-in-polygon으로 직접 판정)으로 바꿔야 하는데, 이 앱은 헤딩업 모드에서
  지도 컨테이너를 CSS로 회전시키는 기능이 있어서 좌표 역변환이 까다로움 — 시도 안 함, 필요시 검토.

## 참고: 관련 API

| 메서드 | 경로 | 설명 |
|---|---|---|
| GET | `/api/polygon/nearby?minLng=&minLat=&maxLng=&maxLat=` | bbox 내 폴리곤(GeoJSON), 응답 헤더 `X-Simplify-Tolerance`에 실제 적용된 단순화 허용오차(m) |
