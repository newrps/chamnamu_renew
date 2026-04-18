<script lang="ts">

    export let show: boolean = false;
    export let lat: number = 0;
    export let lng: number = 0;
    export let adHeight: number = 0;

    interface HourData {
        hour: number;
        label: string;        // "18시"
        temp: number;
        precip: number;
        windspeed: number;    // m/s
        weatherCode: number;
        weatherEmoji: string;
        weatherLabel: string;
        score: number;        // 1~5
        stars: string;
        scoreLabel: string;
        scoreCls: string;
    }

    let moonEmoji = '';
    let moonName = '';
    let moonDesc = '';
    let moonStars = '';
    let moonRatio = 0;

    let hours: HourData[] = [];
    let selectedIdx = 0;
    let loading = false;
    let loadError = false;
    let fetched = false;
    let locationName = '';

    $: selected = hours[selectedIdx] ?? null;

    // ── 달 위상 계산 (수식, API 불필요) ──────────────────────────────────────
    function calcMoonPhase(date: Date): number {
        const knownNewMoon = new Date('2000-01-06T18:14:00Z');
        const period = 29.53059;
        const days = (date.getTime() - knownNewMoon.getTime()) / 86400000;
        return ((days % period) + period) % period / period; // 0~1
    }

    function moonInfo(ratio: number) {
        const dist = Math.min(ratio, 1 - ratio); // 0=삭, 0.5=보름
        const score = Math.max(1, Math.min(5, Math.round((1 - dist * 2) * 5)));

        let emoji: string, name: string, desc: string;
        if (ratio < 0.067 || ratio > 0.933)  { emoji='🌑'; name='삭(그믐)';  desc='달빛 없음 — 채집 최적'; }
        else if (ratio < 0.183)               { emoji='🌒'; name='초승달';    desc='달빛 매우 약함 — 매우 좋음'; }
        else if (ratio < 0.317)               { emoji='🌓'; name='상현달';    desc='달빛 약함 — 좋음'; }
        else if (ratio < 0.433)               { emoji='🌔'; name='보름 직전'; desc='달빛 밝아짐 — 보통'; }
        else if (ratio < 0.567)               { emoji='🌕'; name='보름달';    desc='달빛 밝음 — 채집에 불리'; }
        else if (ratio < 0.683)               { emoji='🌖'; name='보름 직후'; desc='달빛 밝아짐 — 보통'; }
        else if (ratio < 0.817)               { emoji='🌗'; name='하현달';    desc='달빛 약함 — 좋음'; }
        else                                  { emoji='🌘'; name='그믐달';    desc='달빛 거의 없음 — 매우 좋음'; }

        return { emoji, name, desc, score };
    }

    // ── 날씨 코드 → 이모지/이름 ───────────────────────────────────────────────
    function weatherInfo(code: number): { emoji: string; label: string } {
        if (code === 0)           return { emoji: '🌙', label: '맑음' };
        if (code <= 3)            return { emoji: '🌙', label: '구름조금' };
        if (code <= 48)           return { emoji: '🌫️', label: '안개' };
        if (code <= 57)           return { emoji: '🌦️', label: '이슬비' };
        if (code <= 67)           return { emoji: '🌧️', label: '비' };
        if (code <= 77)           return { emoji: '❄️',  label: '눈' };
        if (code <= 82)           return { emoji: '🌦️', label: '소나기' };
        return                           { emoji: '⛈️', label: '뇌우' };
    }

    // ── 채집 점수 계산 (1~5) ──────────────────────────────────────────────────
    function calcScore(temp: number, precip: number, windspeed: number, mRatio: number, month: number): number {
        let s = 0;
        // 기온 (max 3)
        if (temp >= 22 && temp <= 27)                       s += 3;
        else if ((temp >= 18 && temp < 22) || (temp > 27 && temp <= 30)) s += 2;
        else if (temp >= 15 && temp < 18)                   s += 1;
        // 강수 (max 2)
        if (precip === 0)        s += 2;
        else if (precip <= 0.3)  s += 1;
        // 바람 (max 1)
        if (windspeed <= 3)      s += 1;
        else if (windspeed <= 6) s += 0.5;
        // 달 (max 2)
        const dist = Math.min(mRatio, 1 - mRatio);
        s += 2 * (1 - dist * 2);
        // 시기 (max 1)
        if (month >= 6 && month <= 8)          s += 1;
        else if (month === 5 || month === 9)   s += 0.5;
        // 총 max=9 → 1~5
        return Math.max(1, Math.min(5, Math.round((s / 9) * 5)));
    }

    function scoreStyle(score: number): { label: string; cls: string } {
        if (score >= 5) return { label: '최적!', cls: 'best' };
        if (score >= 4) return { label: '좋음',  cls: 'good' };
        if (score >= 3) return { label: '보통',  cls: 'ok' };
        return               { label: '나쁨',   cls: 'bad' };
    }

    // ── Open-Meteo 날씨 로드 ──────────────────────────────────────────────────
    async function fetchLocationName(userLat: number, userLng: number) {
        try {
            const url = `https://nominatim.openstreetmap.org/reverse?lat=${userLat}&lon=${userLng}&format=json&accept-language=ko`;
            const res = await fetch(url, { headers: { 'Accept-Language': 'ko' } });
            if (!res.ok) return;
            const data = await res.json();
            const a = data.address ?? {};
            // 시/도 + 시/군/구 조합
            const parts = [
                a.city ?? a.county ?? a.province ?? a.state,
                a.suburb ?? a.borough ?? a.neighbourhood ?? a.town ?? a.village,
            ].filter(Boolean);
            locationName = parts.slice(0, 2).join(' ');
        } catch { /* 위치명 없어도 예보는 표시 */ }
    }

    async function loadForecast(userLat: number, userLng: number) {
        loading = true;
        loadError = false;
        try {
            const [weatherRes] = await Promise.all([
                fetch(
                    `https://api.open-meteo.com/v1/forecast` +
                    `?latitude=${userLat.toFixed(4)}&longitude=${userLng.toFixed(4)}` +
                    `&hourly=temperature_2m,precipitation,weathercode,windspeed_10m` +
                    `&timezone=Asia%2FSeoul&forecast_days=2`
                ),
                fetchLocationName(userLat, userLng),
            ]);
            const res = weatherRes;
            if (!res.ok) throw new Error('HTTP error');
            const data = await res.json();

            const now = new Date();
            const month = now.getMonth() + 1;
            const ratio = calcMoonPhase(now);
            const mi = moonInfo(ratio);
            moonRatio  = ratio;
            moonEmoji  = mi.emoji;
            moonName   = mi.name;
            moonDesc   = mi.desc;
            moonStars  = '★'.repeat(mi.score) + '☆'.repeat(5 - mi.score);

            // 오늘 18~23시 + 내일 0~2시
            const todayStr    = now.toISOString().slice(0, 10);
            const tomorrow    = new Date(now);
            tomorrow.setDate(tomorrow.getDate() + 1);
            const tomorrowStr = tomorrow.toISOString().slice(0, 10);

            const targets = [18, 19, 20, 21, 22, 23, 0, 1, 2];
            const result: HourData[] = [];

            for (const h of targets) {
                const dateStr = h >= 18 ? todayStr : tomorrowStr;
                const timeStr = `${dateStr}T${String(h).padStart(2, '0')}:00`;
                const idx = (data.hourly.time as string[]).indexOf(timeStr);
                if (idx === -1) continue;

                const temp      = Math.round(data.hourly.temperature_2m[idx]);
                const precip    = data.hourly.precipitation[idx] as number;
                const windKmh   = data.hourly.windspeed_10m[idx] as number;
                const windMs    = windKmh / 3.6;
                const code      = data.hourly.weathercode[idx] as number;
                const wi        = weatherInfo(code);
                const score     = calcScore(temp, precip, windMs, ratio, month);
                const { label: scoreLabel, cls: scoreCls } = scoreStyle(score);

                result.push({
                    hour: h,
                    label: `${h}시`,
                    temp, precip,
                    windspeed: windMs,
                    weatherCode: code,
                    weatherEmoji: wi.emoji,
                    weatherLabel: wi.label,
                    score,
                    stars: '★'.repeat(score) + '☆'.repeat(5 - score),
                    scoreLabel,
                    scoreCls,
                });
            }

            hours = result;
            // 최고점 시간 자동 선택
            selectedIdx = result.reduce((best, h, i) => h.score > result[best].score ? i : best, 0);
            fetched = true;
        } catch {
            loadError = true;
        } finally {
            loading = false;
        }
    }

    async function tryLoad() {
        if (fetched || loading) return;
        if (lat !== 0 && lng !== 0) {
            await loadForecast(lat, lng);
        } else {
            if (!navigator.geolocation) { loadError = true; return; }
            navigator.geolocation.getCurrentPosition(
                pos => loadForecast(pos.coords.latitude, pos.coords.longitude),
                ()  => { loadError = true; },
                { enableHighAccuracy: false, timeout: 6000 }
            );
        }
    }

    // 패널이 열릴 때마다 아직 로드 안 됐으면 로드
    $: if (show && !fetched && !loading) tryLoad();

    // GPS 좌표가 들어오면 (나침반 켰을 때) 로드
    $: if (lat !== 0 && lng !== 0 && !fetched && !loading) loadForecast(lat, lng);
</script>

{#if show}
<div class="cf-panel" style="padding-bottom: {adHeight + 36}px">
    <div class="cf-drag-bar"></div>

    <div class="cf-title">
        🪲 오늘 밤 채집 예보
        <span class="cf-sub">
            {new Date().toLocaleDateString('ko-KR', { month: 'long', day: 'numeric' })}
            {#if locationName} · 📍 {locationName}{/if}
        </span>
    </div>

    {#if loading}
        <div class="cf-loading">날씨 불러오는 중...</div>

    {:else if loadError}
        <div class="cf-loading">
            위치 정보를 불러올 수 없습니다
            <button class="cf-retry" on:click={() => { fetched = false; tryLoad(); }}>재시도</button>
        </div>

    {:else if hours.length > 0}
        <!-- 달 정보 -->
        <div class="cf-moon">
            <span class="cf-moon-emoji">{moonEmoji}</span>
            <div class="cf-moon-text">
                <div class="cf-moon-name">{moonName}</div>
                <div class="cf-moon-desc">{moonDesc}</div>
            </div>
            <div class="cf-moon-stars">{moonStars}</div>
        </div>

        <!-- 시간별 카드 -->
        <div class="cf-scroll">
            {#each hours as h, i}
                <button
                    class="cf-card {h.scoreCls} {i === selectedIdx ? 'sel' : ''}"
                    on:click={() => selectedIdx = i}
                >
                    <div class="cf-card-time">{h.label}</div>
                    <div class="cf-card-wx">{h.weatherEmoji}</div>
                    <div class="cf-card-temp">{h.temp}°</div>
                    <div class="cf-card-stars">{h.stars}</div>
                    <div class="cf-card-label {h.scoreCls}">{h.scoreLabel}</div>
                </button>
            {/each}
        </div>

        <!-- 선택된 시간 상세 -->
        {#if selected}
            <div class="cf-detail">
                <div class="cf-chip {selected.temp >= 22 && selected.temp <= 27 ? 'g' : selected.temp < 18 ? 'b' : ''}">
                    🌡️ <span>{selected.temp}°C</span>
                </div>
                <div class="cf-chip {selected.precip === 0 ? 'g' : 'b'}">
                    {selected.weatherEmoji} <span>{selected.weatherLabel}</span>
                </div>
                <div class="cf-chip {selected.windspeed <= 3 ? 'g' : selected.windspeed <= 6 ? '' : 'b'}">
                    💨 <span>{selected.windspeed.toFixed(1)}m/s</span>
                </div>
                <div class="cf-chip {selected.precip === 0 ? 'g' : 'b'}">
                    💧 <span>{selected.precip.toFixed(1)}mm</span>
                </div>
                <div class="cf-chip {moonRatio < 0.25 || moonRatio > 0.75 ? 'g' : moonRatio > 0.35 && moonRatio < 0.65 ? 'b' : ''}">
                    {moonEmoji} <span>{moonName}</span>
                </div>
            </div>
        {/if}

    {:else}
        <div class="cf-loading">데이터가 없습니다</div>
    {/if}
</div>
{/if}

<style>
    .cf-panel {
        position: fixed;
        bottom: 0; left: 0; right: 0;
        background: rgba(13, 13, 13, 0.94);
        backdrop-filter: blur(20px);
        -webkit-backdrop-filter: blur(20px);
        border-radius: 20px 20px 0 0;
        z-index: 1100;
        padding: 14px 0 36px;
        color: #fff;
    }

    .cf-drag-bar {
        width: 36px; height: 4px;
        background: rgba(255,255,255,0.25);
        border-radius: 2px;
        margin: 0 auto 14px;
    }

    .cf-title {
        font-size: 15px;
        font-weight: 700;
        padding: 0 16px 12px;
        display: flex;
        align-items: baseline;
        gap: 8px;
    }
    .cf-sub {
        font-size: 11px;
        font-weight: 400;
        color: rgba(255,255,255,0.4);
    }

    /* 달 정보 */
    .cf-moon {
        display: flex;
        align-items: center;
        gap: 10px;
        margin: 0 16px 12px;
        background: rgba(255,255,255,0.07);
        border-radius: 14px;
        padding: 10px 14px;
    }
    .cf-moon-emoji { font-size: 30px; flex-shrink: 0; }
    .cf-moon-text  { flex: 1; }
    .cf-moon-name  { font-size: 13px; font-weight: 600; }
    .cf-moon-desc  { font-size: 11px; color: rgba(255,255,255,0.5); margin-top: 2px; }
    .cf-moon-stars { font-size: 11px; color: #ffd54f; flex-shrink: 0; }

    /* 시간별 스크롤 */
    .cf-scroll {
        display: flex;
        gap: 8px;
        overflow-x: auto;
        padding: 4px 16px 8px;
        scrollbar-width: none;
    }
    .cf-scroll::-webkit-scrollbar { display: none; }

    .cf-card {
        flex-shrink: 0;
        width: 68px;
        background: rgba(255,255,255,0.08);
        border-radius: 16px;
        padding: 10px 6px;
        text-align: center;
        border: 1.5px solid transparent;
        cursor: pointer;
        transition: all 0.15s;
        color: #fff;
        font-family: inherit;
    }
    .cf-card.best  { background: rgba(27,94,32,0.45); }
    .cf-card.good  { background: rgba(56,142,60,0.25); }
    .cf-card.bad   { background: rgba(183,28,28,0.2); }
    .cf-card.sel   { border-color: rgba(255,255,255,0.55); }

    .cf-card-time  { font-size: 11px; font-weight: 700; color: rgba(255,255,255,0.65); }
    .cf-card-wx    { font-size: 22px; margin: 5px 0 3px; }
    .cf-card-temp  { font-size: 14px; font-weight: 600; }
    .cf-card-stars { font-size: 8px; color: #ffd54f; margin-top: 5px; letter-spacing: -1px; }
    .cf-card-label { font-size: 10px; font-weight: 700; margin-top: 3px; }

    .cf-card-label.best { color: #81c784; }
    .cf-card-label.good { color: #aed581; }
    .cf-card-label.ok   { color: #fff176; }
    .cf-card-label.bad  { color: #ef9a9a; }

    /* 상세 칩 */
    .cf-detail {
        display: flex;
        flex-wrap: wrap;
        gap: 7px;
        margin: 4px 16px 0;
    }
    .cf-chip {
        background: rgba(255,255,255,0.09);
        border-radius: 20px;
        padding: 5px 12px;
        font-size: 12px;
        display: flex;
        align-items: center;
        gap: 5px;
    }
    .cf-chip.g span { color: #81c784; font-weight: 700; }
    .cf-chip.b span { color: #ef9a9a; font-weight: 700; }
    .cf-chip   span { font-weight: 700; }

    /* 로딩/에러 */
    .cf-loading {
        text-align: center;
        color: rgba(255,255,255,0.45);
        padding: 28px 20px;
        font-size: 14px;
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 10px;
    }
    .cf-retry {
        background: rgba(255,255,255,0.12);
        color: #fff;
        border: none;
        border-radius: 20px;
        padding: 6px 18px;
        font-size: 13px;
        cursor: pointer;
    }
</style>
