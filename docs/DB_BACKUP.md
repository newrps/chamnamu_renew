# DB 백업 가이드

## 백업 위치

| 위치 | 경로 |
|---|---|
| NAS | `/volume1/docker/chamnamu_renew/backups/chanamudb_YYYYMMDD.dump` |
| PC | `C:\git\chamnamu_renew\db_backups\chanamudb_YYYYMMDD.dump` (git 추적 안 함, `.gitignore`에 `db_backups/` 등록됨) |

- 포맷: `pg_dump -Fc` (custom format, 압축됨). `pg_restore`로만 복구 가능 (plain SQL 아님).
- DB 컨테이너: `chamnamu_renew-db-1` (postgis/postgis:17-3.5), NAS 접속은 `ssh -p 56822 newrps@192.168.123.110`.
- ⚠️ postgres 데이터는 docker named volume(`chamnamu_renew_pgdata`)에 있고 실제 경로는 `/volume1/@docker/volumes/chamnamu_renew_pgdata/_data`임. NAS의 "docker 공유폴더" 일일 스냅샷은 `/volume1/docker`만 커버해서 **이 데이터를 백업해주지 않음** — 그래서 pg_dump를 따로 떠서 보관해야 함.
- 자동화(주기적 백업 스케줄)는 아직 설정 안 됨. 필요하면 NAS Task Scheduler(cron)에 아래 "새 백업 만들기" 명령을 등록하면 됨.

## 새 백업 만들기

NAS에 SSH 접속 후:

```bash
DATE=$(date +%Y%m%d)
docker exec chamnamu_renew-db-1 pg_dump -U newrps -Fc -d chanamudb -f /tmp/chanamudb_$DATE.dump
docker cp chamnamu_renew-db-1:/tmp/chanamudb_$DATE.dump /volume1/docker/chamnamu_renew/backups/chanamudb_$DATE.dump
docker exec chamnamu_renew-db-1 rm /tmp/chanamudb_$DATE.dump
```

PC로 내려받기 (Windows에서, legacy scp 모드 `-O` 필요 — 안 붙이면 "subsystem request failed" 에러 남):

```bash
scp -O -P 56822 newrps@192.168.123.110:/volume1/docker/chamnamu_renew/backups/chanamudb_$DATE.dump "C:/git/chamnamu_renew/db_backups/chanamudb_$DATE.dump"
```

## 백업 복구하기

**주의: `-c` 옵션은 기존 객체를 DROP 후 재생성함 — 운영 DB에 실행 시 되돌릴 수 없으니 대상 확인 필수.**

NAS의 덤프 파일로 복구:

```bash
docker cp /volume1/docker/chamnamu_renew/backups/chanamudb_YYYYMMDD.dump chamnamu_renew-db-1:/tmp/restore.dump
docker exec chamnamu_renew-db-1 pg_restore -U newrps -d chanamudb -c /tmp/restore.dump
docker exec chamnamu_renew-db-1 rm /tmp/restore.dump
```

PC에 있는 덤프 파일로 복구하려면 먼저 NAS로 올린 뒤 위 순서를 따르면 됨:

```bash
scp -O -P 56822 "C:/git/chamnamu_renew/db_backups/chanamudb_YYYYMMDD.dump" newrps@192.168.123.110:/volume1/docker/chamnamu_renew/backups/
```

복구 후에는 프론트에 반영되도록 머티리얼라이즈드 뷰도 다시 갱신해야 함:

```bash
docker exec chamnamu_renew-db-1 psql -U newrps -d chanamudb -c "REFRESH MATERIALIZED VIEW chamnamu_tree;"
```

⚠️ `chamnamu_renew2`(Martin+MapLibre 실험판)가 같이 떠 있다면, Martin이 타일을 인메모리에 캐싱하고 있어서(기본 256MB) refresh해도 예전 폴리곤이 계속 보일 수 있음 — `docker compose -f chamnamu_renew2/docker-compose.yml restart martin`으로 캐시를 비워줘야 함.

## 특정 테이블만 백업/복구하고 싶을 때

전체 덤프 대신 테이블 단위로 빠르게 스냅샷 뜨고 싶을 때 (`border_dmz` 예시):

```sql
-- 백업 (원본 보존용 사본 테이블 생성)
CREATE TABLE border_dmz_backup_20260815 AS SELECT * FROM border_dmz;

-- 복구 (백업 테이블 내용으로 되돌리기)
TRUNCATE border_dmz;
INSERT INTO border_dmz SELECT * FROM border_dmz_backup_20260815;
REFRESH MATERIALIZED VIEW chamnamu_tree;
```

---

## 참고: 2026-08-15 border_dmz 좌표 보정 작업

접경지역 임상도 PDF(산림청 제공, `E:\산림공간정보 자료제공\접경지역 임상도\`)를 QGIS로 디지타이징해서 `border_dmz` 테이블에 넣었는데, 강화도 죽산포구 인근 폴리곤이 실제 위치보다 내륙 쪽에 있는 문제를 발견함.

**진단**: PDF가 벡터 데이터라 PyMuPDF로 도곽 눈금 텍스트(예: `126°13'30"E`)와 프레임 사각형의 실제 PDF 좌표를 뽑아서 픽셀→위경도 변환식을 만들고, 폴리곤 벡터 좌표를 직접 추출해 DB 값과 대조함. 교동도 시트 4곳 + 완전히 다른 지역 시트 1곳에서 검증한 결과, **경도는 정상이고 위도만 일정하게(EPSG:5179 기준 약 456m) 북쪽으로 밀려있는 프로젝트 전체 공통 오차**로 확인됨.

**수정**: `border_dmz_backup_20260815` 테이블에 원본을 백업해두고,

```sql
UPDATE border_dmz SET wkb_geometry = ST_Translate(wkb_geometry, 0, -456);
REFRESH MATERIALIZED VIEW chamnamu_tree;
```

로 전체 15,589건을 일괄 보정함. `forestmap_border_staging`(원본 디지타이징 데이터, 4326, 필터링 전)은 프론트에 노출되지 않는 원시 데이터라 이번엔 수정하지 않음 — 이후 새로 디지타이징한 데이터가 같은 오차를 가지고 들어오는지는 다시 확인 필요.
