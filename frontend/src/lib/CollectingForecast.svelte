<script lang="ts">
    export let show: boolean = false;
    export let lat: number = 0;
    export let lng: number = 0;
    export let adHeight: number = 0;
    export let onclose: (() => void) | undefined = undefined;

    // ── 드래그로 닫기 ─────────────────────────────────────────────────────────
    let dragStartY = 0;
    let dragY = 0;
    let isDragging = false;

    function onDragStart(e: TouchEvent | MouseEvent) {
        dragStartY = e instanceof TouchEvent ? e.touches[0].clientY : e.clientY;
        isDragging = true;
        dragY = 0;
        document.addEventListener('touchmove', onDragMove, { passive: true });
        document.addEventListener('touchend', onDragEnd, { once: true });
        document.addEventListener('mousemove', onDragMove);
        document.addEventListener('mouseup', onDragEnd, { once: true });
    }

    function onDragMove(e: TouchEvent | MouseEvent) {
        const y = e instanceof TouchEvent ? e.touches[0].clientY : e.clientY;
        dragY = Math.max(0, y - dragStartY); // 아래 방향만
    }

    function onDragEnd() {
        isDragging = false;
        document.removeEventListener('touchmove', onDragMove);
        document.removeEventListener('mousemove', onDragMove);
        if (dragY > 80) {
            dragY = window.innerHeight; // 화면 아래로 날려서 닫힘
            setTimeout(() => { onclose?.(); dragY = 0; }, 280);
        } else {
            dragY = 0; // 제자리로 복귀
        }
    }

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
    let locationErrorMsg = '위치를 가져오지 못했습니다.\nGPS 신호를 확인하거나 나침반을 먼저 켜보세요.';
    let lastFetchedLat = 0;
    let lastFetchedLng = 0;
    let showMoonCalendar = false;

    $: selected = hours[selectedIdx] ?? null;

    // ── 달력 데이터 (30일) ────────────────────────────────────────────────────
    interface DayData {
        date: Date;
        dateLabel: string;
        dayLabel: string;
        emoji: string;
        name: string;
        ratio: number;
        moonScore: number;
    }

    function buildMoonCalendar(numDays = 30): DayData[] {
        const today = new Date();
        today.setHours(20, 0, 0, 0);
        const weekDays = ['일', '월', '화', '수', '목', '금', '토'];
        const days: DayData[] = [];

        for (let i = 0; i < numDays; i++) {
            const d = new Date(today);
            d.setDate(d.getDate() + i);
            const month = d.getMonth() + 1;
            const ratio = calcMoonPhase(d);
            const mi = moonInfo(ratio);
            const dist = Math.min(ratio, 1 - ratio);
            const mScore = Math.max(1, Math.min(5, Math.round((1 - dist * 2) * 5)));
            const seasonBonus = (month >= 6 && month <= 8) ? 1 : (month === 5 || month === 9) ? 0.5 : 0;
            const total = Math.min(5, mScore + seasonBonus);

            days.push({
                date: d,
                dateLabel: `${d.getMonth() + 1}/${d.getDate()}`,
                dayLabel: weekDays[d.getDay()],
                emoji: mi.emoji,
                name: mi.name,
                ratio,
                moonScore: Math.round(total),
            });
        }
        return days;
    }

    function onCalendarScroll(e: Event) {
        const el = e.target as HTMLElement;
        if (el.scrollHeight - el.scrollTop - el.clientHeight < 60) {
            const next = calendarDays.length + 30;
            if (next <= 365) calendarDays = buildMoonCalendar(next);
        }
    }

    let calendarDays: DayData[] = [];
    $: if (showMoonCalendar && calendarDays.length === 0) calendarDays = buildMoonCalendar(30);

    // ── 달 위상 계산 (수식, API 불필요) ──────────────────────────────────────
    function calcMoonPhase(date: Date): number {
        const jd = date.getTime() / 86400000 + 2440587.5;
        const T = (jd - 2451545.0) / 36525.0;
        const rad = Math.PI / 180;
        const D  = ((297.85036 + 445267.111480 * T - 0.0019142 * T * T) % 360 + 360) % 360;
        const M  = ((357.52772 +  35999.050340 * T - 0.0001603 * T * T) % 360 + 360) % 360;
        const Mp = ((134.96298 + 477198.867398 * T + 0.0086972 * T * T) % 360 + 360) % 360;
        // 교란항 보정으로 실제 신월/보름 시각을 약 2시간 오차까지 줄임 (d5.co.kr 기준 일치)
        const Dc = D
            + 6.289 * Math.sin(Mp * rad)
            - 2.100 * Math.sin(M  * rad)
            + 1.274 * Math.sin((2 * D - Mp) * rad)
            + 0.658 * Math.sin(2 * D  * rad)
            + 0.214 * Math.sin(2 * Mp * rad)
            + 0.110 * Math.sin(D  * rad);
        return ((Dc % 360) + 360) % 360 / 360;
    }

    function moonInfo(ratio: number) {
        const dist = Math.min(ratio, 1 - ratio); // 0=삭, 0.5=보름
        const score = Math.max(1, Math.min(5, Math.round((1 - dist * 2) * 5)));

        let emoji: string, name: string, desc: string;
        // 상현/보름/하현/삭 은 1일 ±6° 좁은 띠, 사이는 긴 구간 (d5.co.kr 방식)
        if (ratio < 0.017 || ratio > 0.983)  { emoji='🌑'; name='삭(그믐)';  desc='달빛 없음 — 채집 최적'; }
        else if (ratio < 0.233)               { emoji='🌒'; name='초승달';    desc='달빛 매우 약함 — 매우 좋음'; }
        else if (ratio < 0.267)               { emoji='🌓'; name='상현달';    desc='달빛 약함 — 좋음'; }
        else if (ratio < 0.483)               { emoji='🌔'; name='보름 직전'; desc='달빛 밝아짐 — 보통'; }
        else if (ratio < 0.517)               { emoji='🌕'; name='보름달';    desc='달빛 밝음 — 채집에 불리'; }
        else if (ratio < 0.733)               { emoji='🌖'; name='보름 직후'; desc='달빛 밝아짐 — 보통'; }
        else if (ratio < 0.767)               { emoji='🌗'; name='하현달';    desc='달빛 약함 — 좋음'; }
        else                                  { emoji='🌘'; name='그믐달';    desc='달빛 거의 없음 — 매우 좋음'; }

        return { emoji, name, desc, score };
    }

    // ── 날씨 코드 → 이모지/이름 ───────────────────────────────────────────────
    function weatherInfo(code: number): { emoji: string; label: string } {
        if (code === 0)           return { emoji: '☀️', label: '맑음' };
        if (code <= 3)            return { emoji: '🌤️', label: '구름조금' };
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

            // 현재 시간부터 24시간
            const todayStr    = now.toISOString().slice(0, 10);
            const tomorrow    = new Date(now);
            tomorrow.setDate(tomorrow.getDate() + 1);
            const tomorrowStr = tomorrow.toISOString().slice(0, 10);

            const currentHour = now.getHours();
            const result: HourData[] = [];

            for (let i = 0; i < 24; i++) {
                const totalH  = currentHour + i;
                const h       = totalH % 24;
                const nextDay = totalH >= 24;
                const dateStr = nextDay ? tomorrowStr : todayStr;
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
                    label: nextDay && h === 0 ? '자정' : `${h}시`,
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
            loading = true; // GPS 응답 대기 중에도 로딩 표시
            navigator.geolocation.getCurrentPosition(
                pos => loadForecast(pos.coords.latitude, pos.coords.longitude),
                (err) => {
                    loading = false;
                    loadError = true;
                    if (err.code === err.PERMISSION_DENIED) {
                        locationErrorMsg = '위치 권한이 거부됐습니다.\n브라우저 설정에서 위치 접근을 허용해 주세요.';
                    } else {
                        locationErrorMsg = '위치를 가져오지 못했습니다.\nGPS 신호를 확인하거나 나침반을 먼저 켜보세요.';
                    }
                },
                { enableHighAccuracy: false, timeout: 8000, maximumAge: 60000 }
            );
        }
    }

    // 패널이 열릴 때마다 아직 로드 안 됐으면 로드
    $: if (show && !fetched && !loading) tryLoad();

    // 좌표가 바뀌면 재조회 (패널이 열려 있을 때만, 닫혀 있으면 다음 오픈 때 tryLoad가 처리)
    $: if (show && lat !== 0 && lng !== 0 && !loading) {
        const moved = Math.abs(lat - lastFetchedLat) + Math.abs(lng - lastFetchedLng) > 0.01;
        if (moved) {
            lastFetchedLat = lat;
            lastFetchedLng = lng;
            fetched = false;
            locationName = '';
            loadForecast(lat, lng);
        }
    }
</script>

{#if show}
<div class="cf-backdrop" on:click={() => onclose?.()} role="presentation"></div>
<div
    class="cf-panel {isDragging ? 'dragging' : ''}"
    style="padding-bottom: {adHeight + 16}px; transform: translateY({dragY}px)"
>
    <div class="cf-drag-handle"
        on:touchstart={onDragStart}
        on:mousedown={onDragStart}
        role="button"
        tabindex="-1"
        aria-label="패널 닫기"
    >
        <div class="cf-drag-bar"></div>
    </div>

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
            <span style="white-space: pre-line; text-align: center;">{locationErrorMsg}</span>
            <button class="cf-retry" on:click={() => { loadError = false; fetched = false; tryLoad(); }}>재시도</button>
        </div>

    {:else if hours.length > 0}
        {#if showMoonCalendar}
            <!-- ── 한달 달 달력 ── -->
            <div class="cf-cal-header">
                <button class="cf-cal-back" on:click={() => showMoonCalendar = false}>← 예보로</button>
                <span>30일 달 달력</span>
            </div>
            <div class="cf-cal-list" on:scroll={onCalendarScroll}>
                {#each calendarDays as d, i}
                    <div class="cf-cal-row {i === 0 ? 'today' : ''}">
                        <span class="cf-cal-date">{d.dateLabel}<span class="cf-cal-day">{d.dayLabel}</span></span>
                        <span class="cf-cal-emoji">{d.emoji}</span>
                        <span class="cf-cal-name">{d.name}</span>
                        <span class="cf-cal-stars score-{d.moonScore}">{'★'.repeat(d.moonScore)}{'☆'.repeat(5 - d.moonScore)}</span>
                    </div>
                {/each}
            </div>

        {:else}
            <!-- ── 달 정보 (클릭하면 달력) ── -->
            <button class="cf-moon" on:click={() => showMoonCalendar = true} title="한달 달력 보기">
                <span class="cf-moon-emoji">{moonEmoji}</span>
                <div class="cf-moon-text">
                    <div class="cf-moon-name">{moonName}</div>
                    <div class="cf-moon-desc">{moonDesc}</div>
                </div>
                <div class="cf-moon-right">
                    <div class="cf-moon-stars">{moonStars}</div>
                    <div class="cf-moon-hint">달력 →</div>
                </div>
            </button>

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
        {/if}

    {:else}
        <div class="cf-loading">데이터가 없습니다</div>
    {/if}
</div>
{/if}

<style>
    .cf-backdrop {
        position: fixed;
        inset: 0;
        z-index: 1099;
        background: transparent;
    }

    .cf-panel {
        position: fixed;
        bottom: 0; left: 0; right: 0;
        background: rgba(13, 13, 13, 0.94);
        backdrop-filter: blur(20px);
        -webkit-backdrop-filter: blur(20px);
        border-radius: 20px 20px 0 0;
        z-index: 1100;
        padding: 0 0 36px;
        color: #fff;
        transition: transform 0.28s cubic-bezier(0.32, 0.72, 0, 1);
        will-change: transform;
    }
    .cf-panel.dragging {
        transition: none;
    }

    /* 터치 히트 영역은 넓게, 시각적 막대는 얇게 */
    .cf-drag-handle {
        padding: 14px 0 10px;
        cursor: grab;
        display: flex;
        justify-content: center;
        align-items: center;
        user-select: none;
    }
    .cf-drag-bar {
        width: 36px; height: 4px;
        background: rgba(255,255,255,0.25);
        border-radius: 2px;
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
        border: none;
        color: #fff;
        font-family: inherit;
        text-align: left;
        cursor: pointer;
        width: calc(100% - 32px);
        transition: background 0.15s;
    }
    .cf-moon:active { background: rgba(255,255,255,0.13); }
    .cf-moon-emoji { font-size: 30px; flex-shrink: 0; }
    .cf-moon-text  { flex: 1; }
    .cf-moon-name  { font-size: 13px; font-weight: 600; }
    .cf-moon-desc  { font-size: 11px; color: rgba(255,255,255,0.5); margin-top: 2px; }
    .cf-moon-right { display: flex; flex-direction: column; align-items: flex-end; gap: 3px; flex-shrink: 0; }
    .cf-moon-stars { font-size: 11px; color: #ffd54f; }
    .cf-moon-hint  { font-size: 10px; color: rgba(255,255,255,0.35); }

    /* 달력 */
    .cf-cal-header {
        display: flex;
        align-items: center;
        gap: 12px;
        padding: 0 16px 10px;
        font-size: 14px;
        font-weight: 700;
    }
    .cf-cal-back {
        background: rgba(255,255,255,0.1);
        border: none;
        color: #fff;
        border-radius: 20px;
        padding: 4px 12px;
        font-size: 12px;
        cursor: pointer;
        font-family: inherit;
    }
    .cf-cal-list {
        overflow-y: auto;
        max-height: 52vh;
        padding: 0 16px 8px;
        display: flex;
        flex-direction: column;
        gap: 4px;
    }
    .cf-cal-row {
        display: flex;
        align-items: center;
        gap: 10px;
        padding: 7px 12px;
        border-radius: 10px;
        background: rgba(255,255,255,0.05);
    }
    .cf-cal-row.today {
        background: rgba(27,94,32,0.4);
        border: 1px solid rgba(76,175,80,0.4);
    }
    .cf-cal-date { font-size: 13px; font-weight: 600; min-width: 42px; }
    .cf-cal-day  { font-size: 10px; color: rgba(255,255,255,0.45); margin-left: 4px; }
    .cf-cal-emoji { font-size: 20px; min-width: 28px; text-align: center; }
    .cf-cal-name { font-size: 12px; color: rgba(255,255,255,0.7); flex: 1; }
    .cf-cal-stars { font-size: 10px; letter-spacing: -1px; }
    .score-5 { color: #81c784; }
    .score-4 { color: #aed581; }
    .score-3 { color: #fff176; }
    .score-2 { color: #ffb74d; }
    .score-1 { color: #ef9a9a; }

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
        margin: 4px 16px 8px;
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
