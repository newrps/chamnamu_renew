/**
 * 채집 예보 (Open-Meteo) 응답 메모리 캐시 + 백그라운드 prefetch.
 *
 * - 좌표 0.01° (~1km) 그리드 키 → 같은 동네에서 좌표 미세 차이로 캐시 미스 방지
 * - 5분 TTL → 시간별 예보가 시간 단위로 갱신되니 그 안에서는 재사용
 * - prefetch: 지도가 사용자 위치 잡으면 호출, 패널 열기 전 이미 채워둠
 */

const TTL_MS = 5 * 60 * 1000;
const GRID = 0.01;

interface CacheEntry {
    data: unknown;
    fetchedAt: number;
}

const cache = new Map<string, CacheEntry>();
const inflight = new Map<string, Promise<unknown | null>>();

function cacheKey(lat: number, lng: number): string {
    const gLat = Math.round(lat / GRID) * GRID;
    const gLng = Math.round(lng / GRID) * GRID;
    return `${gLat.toFixed(2)},${gLng.toFixed(2)}`;
}

function buildUrl(lat: number, lng: number): string {
    return `https://api.open-meteo.com/v1/forecast`
        + `?latitude=${lat.toFixed(4)}&longitude=${lng.toFixed(4)}`
        + `&hourly=temperature_2m,precipitation,weathercode,windspeed_10m`
        + `&timezone=Asia%2FSeoul&forecast_days=2`;
}

/** 5분 이내 캐시된 데이터를 동기적으로 반환. 없거나 만료면 null. */
export function getCachedForecast(lat: number, lng: number): unknown | null {
    const k = cacheKey(lat, lng);
    const e = cache.get(k);
    if (!e) return null;
    if (Date.now() - e.fetchedAt > TTL_MS) {
        cache.delete(k);
        return null;
    }
    return e.data;
}

/**
 * 캐시 우선, 없으면 fetch. 동시 호출은 dedupe.
 * 실패 시 null 반환 (예외 던지지 않음).
 */
export async function fetchForecastRaw(lat: number, lng: number): Promise<unknown | null> {
    if (lat === 0 && lng === 0) return null;
    const cached = getCachedForecast(lat, lng);
    if (cached) return cached;

    const k = cacheKey(lat, lng);
    const existing = inflight.get(k);
    if (existing) return existing;

    const p = (async () => {
        try {
            const res = await fetch(buildUrl(lat, lng));
            if (!res.ok) return null;
            const data = await res.json();
            cache.set(k, { data, fetchedAt: Date.now() });
            return data;
        } catch {
            return null;
        } finally {
            inflight.delete(k);
        }
    })();
    inflight.set(k, p);
    return p;
}

/** 백그라운드 prefetch. 결과는 캐시에만 저장, 호출자는 await 안 함. */
export function prefetchForecast(lat: number, lng: number): void {
    if (lat === 0 && lng === 0) return;
    if (getCachedForecast(lat, lng)) return;
    void fetchForecastRaw(lat, lng);
}
