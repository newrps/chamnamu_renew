<script context="module" lang="ts">
    declare var kakao: any;
</script>

<script lang="ts">
    import { onMount, createEventDispatcher, tick } from 'svelte';
    import { writable } from 'svelte/store';

    const locating = writable(false);

    const dispatch = createEventDispatcher();
    const VITE_API_BASE_URL: string = import.meta.env.VITE_API_BASE_URL;
	const VITE_KAKAO_MAP_API_KEY: string = import.meta.env.VITE_KAKAO_MAP_API_KEY;
    let map: kakao.maps.Map;
    let ps: kakao.maps.services.Places;
    let mapViewport: HTMLElement;
    let mapContainer: HTMLElement;
    let currentMarker: kakao.maps.Marker | null = null;
    export let legendBottom: number = 136;
    export let overviewHidden = false;
    let overviewExpanded = false;

    const OVERVIEW_GEO = {
        minLng: 124.4, maxLng: 131.2,
        minLat: 32.8, maxLat: 38.8,
        width: 120, height: 160, padding: 6
    };
    const KOREA_MAINLAND: [number, number][] = [
        [126.10, 38.55], [126.72, 38.67], [127.18, 38.30], [128.05, 38.58],
        [128.55, 38.25], [129.05, 37.75], [129.42, 37.12], [129.55, 36.35],
        [129.38, 35.72], [129.10, 35.15], [128.58, 34.88], [127.92, 34.70],
        [127.28, 34.47], [126.55, 34.38], [126.12, 34.72], [126.32, 35.25],
        [125.98, 35.72], [126.30, 36.24], [126.03, 36.72], [126.42, 37.24],
        [126.12, 37.78]
    ];

    function overviewProject(lng: number, lat: number) {
        const { minLng, maxLng, minLat, maxLat, width, height, padding } = OVERVIEW_GEO;
        const clampedLng = Math.max(minLng, Math.min(maxLng, lng));
        const clampedLat = Math.max(minLat, Math.min(maxLat, lat));
        return {
            x: padding + ((clampedLng - minLng) / (maxLng - minLng)) * (width - padding * 2),
            y: padding + ((maxLat - clampedLat) / (maxLat - minLat)) * (height - padding * 2)
        };
    }

    const KOREA_MAINLAND_POINTS = KOREA_MAINLAND
        .map(([lng, lat]) => {
            const point = overviewProject(lng, lat);
            return `${point.x.toFixed(1)},${point.y.toFixed(1)}`;
        })
        .join(' ');
    const JEJU_POINT = overviewProject(126.55, 33.38);
    const ULLEUNG_POINT = overviewProject(130.90, 37.50);

    let overviewCenterPoint = overviewProject(127.1148388815, 37.254971339188);
    let overviewViewport = { x: 0, y: 0, width: 0, height: 0 };
    let overviewUpdateRaf: number | null = null;

    type PolygonBounds = { minLat: number; minLng: number; maxLat: number; maxLng: number };
    type CachedPolygon = {
        polygons: kakao.maps.Polygon[];
        species: string | null;
        lastUsedAt: number;
    };
    type PolygonQueryCache = {
        bounds: PolygonBounds;
        ids: Set<string>;
        lastUsedAt: number;
    };

    let drawnPolygons = new Map<string, CachedPolygon>();
    let activePolygonIds = new Set<string>();
    let polygonQueryCache: PolygonQueryCache[] = [];
    let polygonCacheLimit = 14000;
    let polygonQueryCacheLimit = 3;
    let polygonsFetchedOnce = false;
    let selectedSpecies: string | null = null;
    let nearestSpeciesLoading: string | null = null;
    let nearestSpeciesMessage = '';
    let nearestSpeciesMessageTimer: ReturnType<typeof setTimeout> | null = null;
    let pendingNearestPolygonId: string | null = null;
    let nearestHighlightTimer: ReturnType<typeof setTimeout> | null = null;
    const visitedNearestPolygons = new Map<string, Set<string>>();

    // 참나무류: 채집지도의 핵심이라 선명한 색으로 표시
    const OAK_SPECIES = ['신갈나무', '굴참나무', '상수리나무', '기타참나무류'];
    // 그 외 사슴벌레 관련 수종: 참나무보다 옅은 색으로 구분만 되게 표시
    const OTHER_SPECIES = ['밤나무', '자작나무', '포플러', '오리나무', '벚나무', '물푸레나무', '느티나무', '고로쇠나무', '가시나무', '구실잣밤나무', '서어나무'];

    const SPECIES_COLORS: Record<string, { fill: string; stroke: string }> = {
        // 참나무류: 초록 계열로 통일 (명도/색상만 살짝 다르게 구분)
        '신갈나무':     { fill: '#2e7d32', stroke: '#1e5522' },
        '굴참나무':     { fill: '#66bb6a', stroke: '#4c9350' },
        '상수리나무':   { fill: '#1b5e20', stroke: '#123d15' },
        '기타참나무류': { fill: '#9ccc65', stroke: '#7aa048' },
        // 그 외 수종: 초록과 겹치지 않는 별도 색 계열
        '밤나무':       { fill: '#8d6e3d', stroke: '#6d5430' },
        '자작나무':     { fill: '#b5a480', stroke: '#8f8163' },
        '포플러':       { fill: '#c68a4a', stroke: '#9c6c38' },
        '오리나무':     { fill: '#7c6a52', stroke: '#5f5140' },
        '벚나무':       { fill: '#a9788a', stroke: '#835d6b' },
        '물푸레나무':   { fill: '#78889a', stroke: '#5c6878' },
        '느티나무':     { fill: '#a68a3d', stroke: '#816a2f' },
        '고로쇠나무':   { fill: '#a85c4a', stroke: '#834637' },
        '가시나무':     { fill: '#5c8a86', stroke: '#476b68' },
        '구실잣밤나무': { fill: '#bfa14a', stroke: '#967d38' },
        '서어나무':     { fill: '#8a7a9a', stroke: '#6b5f7a' }
    };
    const DEFAULT_SPECIES_COLOR = { fill: '#898781', stroke: '#6b6963' };
    function colorForSpecies(species: string | null | undefined) {
        return (species && SPECIES_COLORS[species]) || DEFAULT_SPECIES_COLOR;
    }

    function polygonRestingOptions(species: string | null | undefined) {
        const emphasized = selectedSpecies === null || selectedSpecies === species;
        return {
            fillOpacity: emphasized ? 0.4 : 0.07,
            strokeOpacity: emphasized ? 0.9 : 0.18,
            strokeWeight: 1
        };
    }

    function refreshSpeciesHighlight() {
        for (const cached of drawnPolygons.values()) {
            const options = polygonRestingOptions(cached.species);
            cached.polygons.forEach(polygon => (polygon as any).setOptions(options));
        }
    }

    function toggleSpeciesHighlight(species: string) {
        visitedNearestPolygons.delete(species);
        selectedSpecies = selectedSpecies === species ? null : species;
        refreshSpeciesHighlight();
    }

    function showNearestSpeciesMessage(message: string) {
        nearestSpeciesMessage = message;
        if (nearestSpeciesMessageTimer) clearTimeout(nearestSpeciesMessageTimer);
        nearestSpeciesMessageTimer = setTimeout(() => {
            nearestSpeciesMessage = '';
            nearestSpeciesMessageTimer = null;
        }, 3500);
    }
    export const speciesLegendPrimary = OAK_SPECIES.map(name => ({ name, color: SPECIES_COLORS[name].fill }));
    export const speciesLegendMore = OTHER_SPECIES.map(name => ({ name, color: SPECIES_COLORS[name].fill }));
    let legendExpanded = false;

    // 범례 좌우 스와이프로 숨기기/보이기
    export let legendHidden = false;
    export let controlsReady = true;
    let legendSwipeStartX = 0;
    let legendSwiping = false;

    function setLegendHidden(hidden: boolean) {
        if (legendHidden === hidden) return;
        legendHidden = hidden;
        dispatch('legendvisibilitychange', { hidden });
    }

    function setOverviewHidden(hidden: boolean) {
        if (overviewHidden === hidden) return;
        overviewHidden = hidden;
        if (!hidden) overviewExpanded = false;
        dispatch('overviewvisibilitychange', { hidden });
    }

    function updateOverviewMap() {
        if (!map) return;
        const center = map.getCenter() as any;
        const bounds = map.getBounds() as any;
        const sw = bounds.getSouthWest();
        const ne = bounds.getNorthEast();
        overviewCenterPoint = overviewProject(center.getLng(), center.getLat());

        const swPoint = overviewProject(sw.getLng(), sw.getLat());
        const nePoint = overviewProject(ne.getLng(), ne.getLat());
        const rawX = Math.min(swPoint.x, nePoint.x);
        const rawY = Math.min(swPoint.y, nePoint.y);
        overviewViewport = {
            x: Math.min(rawX, OVERVIEW_GEO.width - OVERVIEW_GEO.padding - 2),
            y: Math.min(rawY, OVERVIEW_GEO.height - OVERVIEW_GEO.padding - 2),
            width: Math.max(2, Math.abs(nePoint.x - swPoint.x)),
            height: Math.max(2, Math.abs(swPoint.y - nePoint.y))
        };
    }

    function scheduleOverviewUpdate() {
        if (overviewUpdateRaf !== null) return;
        overviewUpdateRaf = requestAnimationFrame(() => {
            overviewUpdateRaf = null;
            updateOverviewMap();
        });
    }

    function legendSwipeStart(e: TouchEvent | MouseEvent) {
        legendSwipeStartX = e instanceof TouchEvent ? e.touches[0].clientX : (e as MouseEvent).clientX;
        legendSwiping = true;
        document.addEventListener('touchmove', legendSwipeMove, { passive: true });
        document.addEventListener('touchend', legendSwipeEnd, { once: true });
        document.addEventListener('mousemove', legendSwipeMove);
        document.addEventListener('mouseup', legendSwipeEnd, { once: true });
    }

    function legendSwipeMove(e: TouchEvent | MouseEvent) {
        if (!legendSwiping) return;
        const x = e instanceof TouchEvent ? e.touches[0].clientX : (e as MouseEvent).clientX;
        const deltaX = x - legendSwipeStartX;
        if (deltaX < -40 && !legendHidden) {
            setLegendHidden(true);
        } else if (deltaX > 40 && legendHidden) {
            setLegendHidden(false);
        }
    }

    function legendSwipeEnd() {
        legendSwiping = false;
        document.removeEventListener('touchmove', legendSwipeMove);
        document.removeEventListener('mousemove', legendSwipeMove);
    }

    let polygonInfoOverlay: any = null;
    function showPolygonInfo(species: string | null | undefined, position: any) {
        const color = colorForSpecies(species);
        const html = `<div style="
                background: rgba(0,0,0,0.8); color: white; padding: 6px 10px;
                border-radius: 6px; font-size: 12px; white-space: nowrap;
                display: flex; align-items: center; gap: 6px;
                box-shadow: 0 2px 8px rgba(0,0,0,0.3); pointer-events: none;
            "><span style="width:9px;height:9px;border-radius:2px;background:${color.fill};flex-shrink:0;"></span><span>${species || '기타참나무류'}</span></div>`;
        if (!polygonInfoOverlay) {
            polygonInfoOverlay = new kakao.maps.CustomOverlay({
                position, content: html, xAnchor: 0.5, yAnchor: 1.4, zIndex: 300
            });
        } else {
            polygonInfoOverlay.setContent(html);
            polygonInfoOverlay.setPosition(position);
        }
        polygonInfoOverlay.setMap(map);
    }
    function hidePolygonInfo() {
        if (polygonInfoOverlay) polygonInfoOverlay.setMap(null);
    }

    type HeadingMode = 'off' | 'north-up' | 'heading-up';
    type ActiveHeadingMode = Exclude<HeadingMode, 'off'>;

    let headingMode: HeadingMode = 'off';
    let isHeadingActive = false;
    // 추적 중 지도를 직접 드래그하면 재중심/회전을 멈추고(다음지도 방식), 나침반 버튼으로 다시 재중심함
    let isFollowing = true;
    let programmaticPanTimer: ReturnType<typeof setTimeout> | null = null;
    let isProgrammaticPan = false;
    let watchId: number | null = null;

    // GPS 위치 추적 중 계속 호출되는 panTo() 자체가 dragstart를 발생시켜서
    // 추적이 스스로 꺼져버리는 문제가 있었음 - 우리 코드가 부른 panTo인지 표시해둠
    function panToPosition(lat: number, lng: number) {
        isProgrammaticPan = true;
        (map as any).panTo(new kakao.maps.LatLng(lat, lng));
        if (programmaticPanTimer) clearTimeout(programmaticPanTimer);
        programmaticPanTimer = setTimeout(() => {
            isProgrammaticPan = false;
        }, 600);
    }
    let savedLocationMarkers: any[] = [];
    let locationOverlay: any = null;
    let overlayElement: HTMLDivElement | null = null;

    function updateHeadingMarkerAppearance() {
        if (!overlayElement) return;
        const cone = overlayElement.querySelector<SVGElement>('[data-heading-cone]');
        const arrow = overlayElement.querySelector<SVGElement>('[data-heading-arrow]');
        if (cone) cone.style.display = headingMode === 'heading-up' ? '' : 'none';
        if (arrow) arrow.style.display = headingMode === 'north-up' ? '' : 'none';
    }

    let currentHeading = 0;
    let continuousHeading = 0;
    let appliedRotationAngle = 0; // mapContainer에 실제로 적용된 CSS 회전각 (드래그 보정 계산에 사용)
    let rafId: number | null = null;
    let interactionUnlockTimer: ReturnType<typeof setTimeout> | null = null;
    let interactionShieldActive = false;
    const activeShieldPointers = new Set<number>();

    // 뷰포트 대각선 길이의 정사각형 지도를 렌더링하면 어떤 각도로 돌려도 모서리가 비지 않는다.
    // 기존 CSS scale 보정을 없애 헤딩업 -> 북쪽 고정 전환 시 발생하던 확대/축소 튐도 제거한다.
    function sizeRotatableMapContainer(): boolean {
        if (!mapViewport || !mapContainer) return false;
        const side = Math.ceil(Math.hypot(mapViewport.clientWidth, mapViewport.clientHeight));
        if (side <= 0 || mapContainer.style.width === `${side}px`) return false;
        mapContainer.style.width = `${side}px`;
        mapContainer.style.height = `${side}px`;
        mapContainer.style.marginLeft = `${-side / 2}px`;
        mapContainer.style.marginTop = `${-side / 2}px`;
        return true;
    }
    let currentLat = 0;
    let currentLng = 0;
    let deviceOrientationAbsoluteHandler: ((e: any) => void) | null = null;
    let deviceOrientationFallbackHandler: ((e: any) => void) | null = null;
    let absoluteOrientationReceived = false;
    let lastHeadingDispatch = 0;

    let lastPolygonUpdate = 0;
    let lastCenter: { lat: number, lng: number } | null = null;
    let fetchAbortController: AbortController | null = null;

    function configurePolygonCacheForDevice() {
        const nav = navigator as Navigator & { deviceMemory?: number };
        const memory = typeof nav.deviceMemory === 'number' ? nav.deviceMemory : null;
        const cores = navigator.hardwareConcurrency || 2;

        if (memory !== null && memory <= 2) {
            polygonCacheLimit = 12000;
            polygonQueryCacheLimit = 2;
        } else if (memory !== null && memory <= 4) {
            polygonCacheLimit = 16000;
            polygonQueryCacheLimit = 4;
        } else if (memory !== null) {
            polygonCacheLimit = 24000;
            polygonQueryCacheLimit = 7;
        } else if (cores <= 4) {
            polygonCacheLimit = 13000;
            polygonQueryCacheLimit = 2;
        } else {
            polygonCacheLimit = 18000;
            polygonQueryCacheLimit = 4;
        }
    }

    function parseGeoJSON(geometry: any): kakao.maps.LatLng[][] {
        if (!geometry) return [];
        const paths: kakao.maps.LatLng[][] = [];
        if (geometry.type === 'MultiPolygon') {
            for (const polygon of geometry.coordinates) {
                const ring: [number, number][] = polygon[0];
                paths.push(ring.map(([lng, lat]) => new kakao.maps.LatLng(lat, lng)));
            }
        } else if (geometry.type === 'Polygon') {
            const ring: [number, number][] = geometry.coordinates[0];
            paths.push(ring.map(([lng, lat]) => new kakao.maps.LatLng(lat, lng)));
        }
        return paths;
    }

    function applyContainerTransform(angle: number, withTransition: boolean) {
        if (!mapContainer) return;
        mapContainer.style.transition = withTransition ? 'transform 0.2s linear' : 'none';
        mapContainer.style.transform = `rotate(${angle}deg)`;
        mapContainer.style.transformOrigin = '50% 50%';
        appliedRotationAngle = angle;
    }

    function normalizeHeading(heading: number): number {
        return ((heading % 360) + 360) % 360;
    }

    // 모든 센서 값을 "정북=0, 동쪽=90"인 나침반 각도로 통일한다.
    function headingFromOrientation(event: any): number | null {
        if (typeof event.webkitCompassHeading === 'number') {
            return normalizeHeading(event.webkitCompassHeading);
        }
        if (typeof event.alpha !== 'number') return null;

        // DeviceOrientation alpha는 정북 기준 반시계 방향 각도이므로 나침반의 시계 방향 각도로 변환한다.
        // 가로 화면에서는 사용자가 보고 있는 화면 위쪽이 달라지므로 화면 회전각도 함께 보정한다.
        const screenAngle = Number(window.screen?.orientation?.angle ?? (window as any).orientation ?? 0);
        return normalizeHeading(360 - event.alpha + screenAngle);
    }

    function applyMapRotation(heading: number) {
        currentHeading = normalizeHeading(heading);
        const current = ((continuousHeading % 360) + 360) % 360;
        let diff = currentHeading - current;
        if (diff > 180) diff -= 360;
        if (diff < -180) diff += 360;
        continuousHeading += diff;

        // 재중심(팔로잉) 여부와 상관없이 현재 모드의 지도/마커 회전을 항상 실시간으로 반영한다.
        if (rafId !== null) cancelAnimationFrame(rafId);
        const angle = continuousHeading;
        const mapAngle = headingMode === 'heading-up' ? -angle : 0;
        rafId = requestAnimationFrame(() => {
            if (overlayElement) {
                overlayElement.style.transition = 'transform 0.2s linear';
                // 북쪽 고정에서는 바라보는 방향을 표시하고, 헤딩업에서는 지도 회전을 상쇄해 항상 위를 향한다.
                overlayElement.style.transform = `rotate(${angle}deg)`;
            }
            applyContainerTransform(mapAngle, true);
            rafId = null;
        });
    }

    function createOrUpdateLocationOverlay(lat: number, lng: number) {
        const pos = new kakao.maps.LatLng(lat, lng);
        if (!locationOverlay) {
            const el = document.createElement('div');
            el.style.cssText = 'pointer-events:none;transform-origin:50% 75%;';
            el.innerHTML = `<svg width="36" height="47" viewBox="0 0 56 72" xmlns="http://www.w3.org/2000/svg">
                <polygon data-heading-cone points="28,46 2,2 54,2" fill="rgba(66,133,244,0.35)" stroke="rgba(66,133,244,0.5)" stroke-width="1" stroke-linejoin="round"/>
                <polygon data-heading-arrow points="28,23 18,45 38,45" fill="#4285f4" stroke="white" stroke-width="2" stroke-linejoin="round"/>
                <circle cx="28" cy="54" r="15" fill="white" style="filter:drop-shadow(0 2px 6px rgba(0,0,0,0.35))"/>
                <circle cx="28" cy="54" r="11" fill="#4285f4"/>
                <circle cx="28" cy="54" r="4" fill="white"/>
            </svg>`;
            overlayElement = el;
            updateHeadingMarkerAppearance();
            if (continuousHeading !== 0) {
                el.style.transform = `rotate(${continuousHeading}deg)`;
            }
            locationOverlay = new kakao.maps.CustomOverlay({
                position: pos,
                content: el,
                xAnchor: 0.5,
                yAnchor: 0.75,
                zIndex: 100
            });
            locationOverlay.setMap(map);
        } else {
            locationOverlay.setPosition(pos);
        }
    }

    function startOrientationTracking() {
        if (!window.isSecureContext) {
            console.warn('[나침반] 비보안 컨텍스트(HTTP)에서는 방향 센서를 사용할 수 없습니다.');
            alert('나침반 기능은 HTTPS 환경에서만 동작합니다. 현재 HTTP로 접속 중입니다.');
            return;
        }

        absoluteOrientationReceived = false;

        const handleAbsolute = (event: DeviceOrientationEvent) => {
            const heading = headingFromOrientation(event);
            if (heading === null) return;
            absoluteOrientationReceived = true;
            currentHeading = heading;
            applyMapRotation(heading);
            const now = Date.now();
            if (now - lastHeadingDispatch > 200) {
                lastHeadingDispatch = now;
                dispatch('headingupdate', { lat: currentLat, lng: currentLng, heading });
            }
        };

        const handleFallback = (event: any) => {
            if (absoluteOrientationReceived) return;
            const heading = headingFromOrientation(event);
            if (heading === null) return;
            currentHeading = heading;
            applyMapRotation(heading);
            const now = Date.now();
            if (now - lastHeadingDispatch > 200) {
                lastHeadingDispatch = now;
                dispatch('headingupdate', { lat: currentLat, lng: currentLng, heading });
            }
        };

        deviceOrientationAbsoluteHandler = handleAbsolute;
        deviceOrientationFallbackHandler = handleFallback;

        window.addEventListener('deviceorientationabsolute', handleAbsolute as any, true);
        window.addEventListener('deviceorientation', handleFallback, true);
    }

    function updatePolygonsIfNeeded(lat: number, lng: number) {
        const now = Date.now();
        if (now - lastPolygonUpdate < 3000) return;
        if (lastCenter) {
            const distance = getDistance(lastCenter.lat, lastCenter.lng, lat, lng);
            if (distance < 0.001) return;
        }
        lastCenter = { lat, lng };
        lastPolygonUpdate = now;
        fetchAndDrawPolygons();
    }

    export function startHeading(mode: ActiveHeadingMode = 'north-up') {
        if (!navigator.geolocation) {
            alert('이 브라우저에서는 위치 서비스를 지원하지 않습니다.');
            return;
        }

        if (isHeadingActive) {
            setHeadingMode(mode);
            return;
        }

        headingMode = mode;
        isHeadingActive = true;
        isFollowing = true;
        interactionShieldActive = mode === 'heading-up';
        setMapInteractionEnabled(mode !== 'heading-up');
        dispatch('headingmodechange', { mode: headingMode });
        startOrientationTracking();
        locating.set(true);

        let gpsFixed = false;
        let freshPositionReceived = false;

        const showPosition = (position: GeolocationPosition, updatePolygons: boolean) => {
            if (!isHeadingActive) return;

            if (!gpsFixed) {
                gpsFixed = true;
                dispatch('gpsfixed');
            }
            locating.set(false);

            const lat = position.coords.latitude;
            const lng = position.coords.longitude;
            currentLat = lat;
            currentLng = lng;

            createOrUpdateLocationOverlay(lat, lng);
            if (isFollowing) {
                panToPosition(lat, lng);
                if (updatePolygons) updatePolygonsIfNeeded(lat, lng);
            }

            const gpsHeading = position.coords.heading;
            if (!absoluteOrientationReceived && gpsHeading !== null && gpsHeading !== undefined) {
                currentHeading = gpsHeading;
                applyMapRotation(gpsHeading);
            }

            dispatch('headingupdate', { lat, lng, heading: currentHeading, accuracy: position.coords.accuracy });
        };

        navigator.geolocation.getCurrentPosition(
            (position) => {
                // 캐시된 빠른 위치로 마커를 먼저 보여준다. 먼저 도착한 실시간 위치는 덮어쓰지 않는다.
                if (!freshPositionReceived) showPosition(position, false);
            },
            () => {},
            { enableHighAccuracy: false, maximumAge: 30000, timeout: 3000 }
        );

        watchId = navigator.geolocation.watchPosition(
            (position) => {
                freshPositionReceived = true;
                showPosition(position, true);
            },
            (error) => {
                console.error('위치 정보를 가져오는데 실패했습니다:', error);
                // 빠른 위치가 이미 보이는 상태라면 고정밀 GPS의 일시적 실패로 추적 모드를 종료하지 않는다.
                if (gpsFixed && error.code !== error.PERMISSION_DENIED) return;
                alert('위치 정보를 가져올 수 없습니다.');
                stopHeading();
            },
            { enableHighAccuracy: true, maximumAge: 0, timeout: 10000 }
        );
    }

    // 드래그로 화면을 옮기면 재중심/회전만 멈춤 - 추적 자체(GPS/방향 센서)는 계속 유지
    function pauseFollowing() {
        if (!isFollowing) return;
        isFollowing = false;
        dispatch('followchange', { following: false });
    }

    // 나침반 버튼을 다시 눌러서 재중심 - GPS를 재시작하지 않고 바로 최신 위치/각도로 스냅
    export function resumeFollowing() {
        if (!isHeadingActive) return;
        isFollowing = true;
        dispatch('followchange', { following: true });
        panToPosition(currentLat, currentLng);
        applyMapRotation(currentHeading);
    }

    export function setHeadingMode(mode: ActiveHeadingMode) {
        if (!isHeadingActive) return;
        headingMode = mode;
        interactionShieldActive = mode === 'heading-up';
        setMapInteractionEnabled(mode !== 'heading-up');
        dispatch('headingmodechange', { mode: headingMode });
        updateHeadingMarkerAppearance();
        applyMapRotation(currentHeading);
    }

    export function stopHeading() {
        headingMode = 'off';
        isHeadingActive = false;
        isFollowing = true;
        locating.set(false);

        if (watchId !== null) {
            navigator.geolocation.clearWatch(watchId);
            watchId = null;
        }

        if (deviceOrientationAbsoluteHandler) {
            window.removeEventListener('deviceorientationabsolute', deviceOrientationAbsoluteHandler as any, true);
            deviceOrientationAbsoluteHandler = null;
        }
        if (deviceOrientationFallbackHandler) {
            window.removeEventListener('deviceorientation', deviceOrientationFallbackHandler, true);
            deviceOrientationFallbackHandler = null;
        }

        if (rafId !== null) {
            cancelAnimationFrame(rafId);
            rafId = null;
        }

        if (locationOverlay) {
            locationOverlay.setMap(null);
            locationOverlay = null;
            overlayElement = null;
        }

        currentHeading = 0;
        continuousHeading = 0;
        appliedRotationAngle = 0;
        if (interactionUnlockTimer) {
            clearTimeout(interactionUnlockTimer);
            interactionUnlockTimer = null;
        }
        activeShieldPointers.clear();
        interactionShieldActive = false;
        setMapInteractionEnabled(true);
        absoluteOrientationReceived = false;
        if (mapContainer) {
            mapContainer.style.transition = '';
            mapContainer.style.transform = 'rotate(0deg)';
        }
        lastPolygonUpdate = 0;
        lastCenter = null;

        dispatch('headingstop');
    }

    function setMapInteractionEnabled(enabled: boolean) {
        if (!map) return;
        (map as any).setDraggable(enabled);
        (map as any).setZoomable(enabled);
    }

    // 헤딩업에서는 카카오맵 자체 제스처를 잠가 좌표계가 바뀌는 도중 드래그/줌이 시작되지 않게 한다.
    // 첫 입력은 북쪽 고정 전환에만 사용하고, 손가락이 완전히 떨어질 시간을 확보한 뒤 지도를 조작한다.
    function switchToNorthUpForInteraction() {
        if (!isHeadingActive || headingMode !== 'heading-up') return;
        if (isFollowing) pauseFollowing();

        headingMode = 'north-up';
        dispatch('headingmodechange', { mode: headingMode });
        updateHeadingMarkerAppearance();
        applyMapRotation(currentHeading);

    }

    function unlockMapInteraction() {
        if (activeShieldPointers.size > 0 || headingMode === 'heading-up') return;
        setMapInteractionEnabled(true);
        interactionShieldActive = false;
    }

    function handleInteractionShieldPointerDown(event: PointerEvent) {
        event.preventDefault();
        event.stopPropagation();
        activeShieldPointers.add(event.pointerId);
        (event.currentTarget as HTMLElement).setPointerCapture?.(event.pointerId);
        switchToNorthUpForInteraction();
    }

    function handleInteractionShieldPointerEnd(event: PointerEvent) {
        event.preventDefault();
        event.stopPropagation();
        activeShieldPointers.delete(event.pointerId);
        const shield = event.currentTarget as HTMLElement;
        if (shield.hasPointerCapture?.(event.pointerId)) {
            shield.releasePointerCapture(event.pointerId);
        }
        if (activeShieldPointers.size === 0) {
            // 현재 pointerup/cancel 흐름이 완전히 끝난 다음 프레임부터 새 제스처를 허용한다.
            requestAnimationFrame(unlockMapInteraction);
        }
    }

    function handleInteractionShieldWheel(event: WheelEvent) {
        event.preventDefault();
        event.stopPropagation();
        switchToNorthUpForInteraction();
        if (interactionUnlockTimer) clearTimeout(interactionUnlockTimer);
        interactionUnlockTimer = setTimeout(() => {
            interactionUnlockTimer = null;
            unlockMapInteraction();
        }, 180);
    }

    function initializeMap() {
        if (typeof kakao !== 'undefined' && kakao.maps) {
            sizeRotatableMapContainer();
            const mapOption = {
                center: new kakao.maps.LatLng(37.254971339188, 127.1148388815),
                level: 3,
                maxLevel: 7
            };
            map = new kakao.maps.Map(mapContainer, mapOption);
            ps = new kakao.maps.services.Places();
            setMapInteractionEnabled(headingMode !== 'heading-up');

            fetchAndDrawPolygons();
            scheduleOverviewUpdate();

            kakao.maps.event.addListener(map, 'bounds_changed', scheduleOverviewUpdate);

            kakao.maps.event.addListener(map, 'dragstart', () => {
                // 우리 코드가 panTo()로 지도를 움직인 것이면(현재위치 추적 중 계속 발생) 무시
                if (isProgrammaticPan) return;
                if (isHeadingActive && isFollowing) pauseFollowing();
            });
            kakao.maps.event.addListener(map, 'dragend', () => {
                fetchAndDrawPolygons();
            });
            kakao.maps.event.addListener(map, 'zoom_changed', () => {
                fetchAndDrawPolygons();
            });
            kakao.maps.event.addListener(map, 'zoom_start', () => {
                if (isHeadingActive && isFollowing) pauseFollowing();
            });
            kakao.maps.event.addListener(map, 'click', hidePolygonInfo);
        } else {
            console.error("카카오맵 API 스크립트가 아직 로드되지 않았습니다.");
        }
    }

    let mapResizeObserver: ResizeObserver | null = null;

    // 뷰포트 크기가 바뀌면 대각선 정사각형을 다시 계산하고 카카오맵 레이아웃을 갱신한다.
    function setupMapResizeHandling() {
        if (!mapViewport || !mapContainer || !map) return;
        mapResizeObserver = new ResizeObserver(() => {
            if (!map || !sizeRotatableMapContainer()) return;
            const center = map.getCenter();
            (map as any).relayout();
            map.setCenter(center);
            if (isHeadingActive) applyMapRotation(currentHeading);
        });
        mapResizeObserver.observe(mapViewport);
    }

    function getDistance(lat1: number, lng1: number, lat2: number, lng2: number): number {
        const dLat = lat1 - lat2;
        const dLng = lng1 - lng2;
        return Math.sqrt(dLat * dLat + dLng * dLng);
    }

    // 뷰포트가 이 정도(위/경도 기준)보다 넓게 확대축소되면 서버에서도 빈 목록을 주므로
    // 굳이 요청을 안 보내고 화면 폴리곤만 정리한다 (백엔드 MAX_BBOX_DEGREES와 맞출 것)
    const MAX_BBOX_DEGREES = 1.0;
    const POLYGON_QUERY_CACHE_TTL = 15 * 60 * 1000;

    function getPaddedBounds() {
        const bounds = map.getBounds() as any;
        const sw = bounds.getSouthWest();
        const ne = bounds.getNorthEast();
        // 패닝 시 화면 가장자리에서 바로 데이터가 끊기지 않도록 15% 여유를 둔다
        const padLat = (ne.getLat() - sw.getLat()) * 0.15;
        const padLng = (ne.getLng() - sw.getLng()) * 0.15;
        return {
            minLat: sw.getLat() - padLat,
            minLng: sw.getLng() - padLng,
            maxLat: ne.getLat() + padLat,
            maxLng: ne.getLng() + padLng
        };
    }

    function boundsContains(outer: PolygonBounds, inner: PolygonBounds) {
        return outer.minLat <= inner.minLat && outer.minLng <= inner.minLng &&
            outer.maxLat >= inner.maxLat && outer.maxLng >= inner.maxLng;
    }

    function hideActivePolygons() {
        for (const id of activePolygonIds) {
            drawnPolygons.get(id)?.polygons.forEach(polygon => polygon.setMap(null));
        }
        activePolygonIds.clear();
        hidePolygonInfo();
    }

    function activatePolygonIds(ids: Set<string>) {
        for (const id of activePolygonIds) {
            if (!ids.has(id)) {
                drawnPolygons.get(id)?.polygons.forEach(polygon => polygon.setMap(null));
            }
        }

        const now = Date.now();
        for (const id of ids) {
            const cached = drawnPolygons.get(id);
            if (!cached) continue;
            cached.lastUsedAt = now;
            if (!activePolygonIds.has(id)) {
                cached.polygons.forEach(polygon => polygon.setMap(map));
            }
        }
        activePolygonIds = new Set(ids);
    }

    function findCachedPolygonQuery(bounds: PolygonBounds) {
        const now = Date.now();
        polygonQueryCache = polygonQueryCache.filter(entry => now - entry.lastUsedAt < POLYGON_QUERY_CACHE_TTL);
        const entry = polygonQueryCache.find(candidate =>
            boundsContains(candidate.bounds, bounds) &&
            Array.from(candidate.ids).every(id => drawnPolygons.has(id))
        );
        if (entry) entry.lastUsedAt = now;
        return entry;
    }

    function rememberPolygonQuery(bounds: PolygonBounds, ids: Set<string>) {
        const now = Date.now();
        polygonQueryCache = polygonQueryCache.filter(entry => !boundsContains(bounds, entry.bounds));
        polygonQueryCache.push({ bounds, ids: new Set(ids), lastUsedAt: now });
        polygonQueryCache.sort((a, b) => b.lastUsedAt - a.lastUsedAt);
        polygonQueryCache = polygonQueryCache.slice(0, polygonQueryCacheLimit);
    }

    function evictUnusedPolygons() {
        if (drawnPolygons.size <= polygonCacheLimit) return;

        const removable = Array.from(drawnPolygons.entries())
            .filter(([id]) => !activePolygonIds.has(id))
            .sort(([, a], [, b]) => a.lastUsedAt - b.lastUsedAt);
        const evictedIds = new Set<string>();

        for (const [id, cached] of removable) {
            if (drawnPolygons.size <= polygonCacheLimit) break;
            cached.polygons.forEach(polygon => polygon.setMap(null));
            drawnPolygons.delete(id);
            evictedIds.add(id);
        }

        if (evictedIds.size > 0) {
            polygonQueryCache = polygonQueryCache.filter(entry =>
                !Array.from(evictedIds).some(id => entry.ids.has(id))
            );
        }
    }

    function adaptPolygonCacheToRenderTime(renderMs: number) {
        if (renderMs < 1200 || drawnPolygons.size <= activePolygonIds.size) return;
        polygonCacheLimit = Math.max(activePolygonIds.size, Math.floor(polygonCacheLimit * 0.8));
        polygonQueryCacheLimit = Math.max(2, polygonQueryCacheLimit - 1);
    }

    function highlightPendingNearestPolygon() {
        if (!pendingNearestPolygonId) return;
        const cached = drawnPolygons.get(pendingNearestPolygonId);
        if (!cached || !activePolygonIds.has(pendingNearestPolygonId)) return;

        if (nearestHighlightTimer) clearTimeout(nearestHighlightTimer);
        cached.polygons.forEach(polygon => (polygon as any).setOptions({
            fillOpacity: 0.9,
            strokeOpacity: 1,
            strokeWeight: 3
        }));
        const highlightedId = pendingNearestPolygonId;
        pendingNearestPolygonId = null;
        nearestHighlightTimer = setTimeout(() => {
            const highlighted = drawnPolygons.get(highlightedId);
            if (highlighted) {
                const options = polygonRestingOptions(highlighted.species);
                highlighted.polygons.forEach(polygon => (polygon as any).setOptions(options));
            }
            nearestHighlightTimer = null;
        }, 2800);
    }

    async function moveToNearestSpecies(species: string) {
        if (!map || nearestSpeciesLoading) return;
        if (selectedSpecies !== species) visitedNearestPolygons.delete(species);
        nearestSpeciesLoading = species;
        selectedSpecies = species;
        refreshSpeciesHighlight();

        const center = map.getCenter() as any;
        const visitedIds = visitedNearestPolygons.get(species) ?? new Set<string>();
        const params = new URLSearchParams({
            species,
            lat: String(center.getLat()),
            lng: String(center.getLng())
        });
        if (visitedIds.size > 0) params.set('excludeIds', Array.from(visitedIds).join(','));

        try {
            const response = await fetch(`${VITE_API_BASE_URL}/api/polygon/nearest?${params}`);
            if (response.status === 404) {
                if (visitedIds.size > 0) {
                    visitedNearestPolygons.delete(species);
                    showNearestSpeciesMessage(`주변의 ${species} 군락을 모두 확인했습니다.`);
                } else {
                    showNearestSpeciesMessage(`${species} 군락을 찾지 못했습니다.`);
                }
                return;
            }
            if (!response.ok) throw new Error(`HTTP ${response.status}`);

            const result = await response.json();
            const resultId = String(result.id);
            visitedIds.add(resultId);
            visitedNearestPolygons.set(species, visitedIds);
            pendingNearestPolygonId = resultId;
            if (isHeadingActive && isFollowing) pauseFollowing();
            map.setCenter(new kakao.maps.LatLng(result.lat, result.lng));
            if ((map as any).getLevel() > 4) (map as any).setLevel(4);

            const distance = Number(result.distance_m);
            const distanceText = distance < 1000
                ? `${Math.round(distance)}m`
                : `${(distance / 1000).toFixed(1)}km`;
            const prefix = visitedIds.size > 1 ? '다음 가까운' : '가장 가까운';
            showNearestSpeciesMessage(`${prefix} ${species} · ${distanceText}`);

            setTimeout(() => fetchAndDrawPolygons(), 120);
        } catch (error) {
            console.error('가까운 수종 검색에 실패했습니다:', error);
            showNearestSpeciesMessage('가까운 군락을 찾는 중 오류가 발생했습니다.');
        } finally {
            nearestSpeciesLoading = null;
        }
    }

    async function fetchAndDrawPolygons() {
        if (!map) return;

        if (fetchAbortController) fetchAbortController.abort();
        fetchAbortController = new AbortController();
        const thisController = fetchAbortController;
        const signal = thisController.signal;

        const requestBounds = getPaddedBounds();
        const { minLat, minLng, maxLat, maxLng } = requestBounds;

        if ((maxLng - minLng) > MAX_BBOX_DEGREES || (maxLat - minLat) > MAX_BBOX_DEGREES) {
            // 너무 축소된 상태 - 전국 단위 조회는 무거우니 건너뛰고 기존 폴리곤만 정리
            hideActivePolygons();
            return;
        }

        const cachedQuery = findCachedPolygonQuery(requestBounds);
        if (cachedQuery) {
            activatePolygonIds(cachedQuery.ids);
            highlightPendingNearestPolygon();
            if (!polygonsFetchedOnce) {
                polygonsFetchedOnce = true;
                dispatch('polygonsfetched');
            }
            return;
        }

        const apiUrl = `${VITE_API_BASE_URL}/api/polygon/nearby?minLng=${minLng}&minLat=${minLat}&maxLng=${maxLng}&maxLat=${maxLat}`;

        let timedOut = false;
        const timeoutId = setTimeout(() => {
            timedOut = true;
            thisController.abort();
        }, 10000);

        try {
            const response = await fetch(apiUrl, { signal });
            clearTimeout(timeoutId);
            if (!response.ok) throw new Error(`HTTP 오류! 상태: ${response.status}`);
            const data = await response.json();

            if (signal.aborted) return;

            const incomingIds = new Set<string>();
            const renderStartedAt = performance.now();

            data.forEach((item: any) => {
                const key = String(item.id);
                incomingIds.add(key);
                const cached = drawnPolygons.get(key);
                if (cached) {
                    cached.lastUsedAt = Date.now();
                    return;
                }
                const paths = parseGeoJSON(item.geometry);
                const color = colorForSpecies(item.species);
                const newPolys: kakao.maps.Polygon[] = [];
                paths.forEach((path: kakao.maps.LatLng[]) => {
                    if (path.length > 0) {
                        const polygon = new kakao.maps.Polygon({
                            path,
                            strokeWeight: 1,
                            strokeColor: color.stroke,
                            strokeOpacity: polygonRestingOptions(item.species).strokeOpacity,
                            fillColor: color.fill,
                            fillOpacity: polygonRestingOptions(item.species).fillOpacity
                        });
                        polygon.setMap(map);
                        kakao.maps.event.addListener(polygon, 'mouseover', (e: any) => {
                            polygon.setOptions({ fillOpacity: 0.8, strokeOpacity: 0.9, strokeWeight: 2 });
                            showPolygonInfo(item.species, e.latLng);
                        });
                        kakao.maps.event.addListener(polygon, 'mousemove', (e: any) => {
                            if (polygonInfoOverlay) polygonInfoOverlay.setPosition(e.latLng);
                        });
                        kakao.maps.event.addListener(polygon, 'mouseout', () => {
                            polygon.setOptions(polygonRestingOptions(item.species));
                            hidePolygonInfo();
                        });
                        let tapHighlightTimeout: ReturnType<typeof setTimeout> | null = null;
                        kakao.maps.event.addListener(polygon, 'click', (e: any) => {
                            kakao.maps.event.preventMap();
                            polygon.setOptions({ fillOpacity: 0.8, strokeOpacity: 0.9, strokeWeight: 2 });
                            showPolygonInfo(item.species, e.latLng);
                            if (tapHighlightTimeout) clearTimeout(tapHighlightTimeout);
                            tapHighlightTimeout = setTimeout(() => {
                                polygon.setOptions(polygonRestingOptions(item.species));
                                hidePolygonInfo();
                            }, 2000);
                        });
                        newPolys.push(polygon);
                    }
                });
                drawnPolygons.set(key, { polygons: newPolys, species: item.species ?? null, lastUsedAt: Date.now() });
            });

            activatePolygonIds(incomingIds);
            rememberPolygonQuery(requestBounds, incomingIds);
            adaptPolygonCacheToRenderTime(performance.now() - renderStartedAt);
            evictUnusedPolygons();
            highlightPendingNearestPolygon();

            if (!polygonsFetchedOnce) {
                polygonsFetchedOnce = true;
                dispatch('polygonsfetched');
            }
        } catch (error: any) {
            clearTimeout(timeoutId);
            if (error?.name === 'AbortError' && !timedOut) return;
            console.error('폴리곤 데이터를 불러오는 데 실패했습니다:', error);
            dispatch('fetcherror');
        }
    }

    export function retryFetchPolygons() {
        fetchAndDrawPolygons();
    }

    export let isSatellite = false;
    export function toggleSatellite() {
        if (!map) return;
        isSatellite = !isSatellite;
        map.setMapTypeId(isSatellite ? kakao.maps.MapTypeId.HYBRID : kakao.maps.MapTypeId.ROADMAP);
    }

    // 로드뷰: 도로 위 파란 선 표시 -> 클릭하면 그 지점 거리뷰(파노라마)를 보여줌
    export let roadviewMode = false;
    let roadviewOpen = false;
    let roadviewContainer: HTMLElement;
    let miniMapContainer: HTMLElement;
    let roadviewClient: any;
    let roadviewInstance: any;
    let roadviewClickListener: any;
    let miniMap: any;
    let miniMapDirEl: HTMLDivElement | null = null;
    let miniMapDirOverlay: any;
    let miniMapPolygons: any[] = [];

    export function toggleRoadview() {
        if (!map) return;
        roadviewMode = !roadviewMode;
        if (roadviewMode) {
            (map as any).addOverlayMapTypeId(kakao.maps.MapTypeId.ROADVIEW);
            roadviewClickListener = kakao.maps.event.addListener(map, 'click', handleRoadviewMapClick);
        } else {
            (map as any).removeOverlayMapTypeId(kakao.maps.MapTypeId.ROADVIEW);
            if (roadviewClickListener) kakao.maps.event.removeListener(map, 'click', roadviewClickListener);
            closeRoadview();
        }
    }

    function ensureMiniMapDirOverlay(position: any) {
        if (!miniMapDirOverlay) {
            miniMapDirEl = document.createElement('div');
            miniMapDirEl.style.cssText = 'pointer-events:none;transform-origin:50% 50%;';
            miniMapDirEl.innerHTML = `<svg width="34" height="34" viewBox="0 0 34 34">
                <polygon points="17,3 6,30 17,23 28,30" fill="rgba(255,80,0,0.9)" stroke="white" stroke-width="1.5" stroke-linejoin="round"/>
            </svg>`;
            miniMapDirOverlay = new kakao.maps.CustomOverlay({
                position, content: miniMapDirEl, xAnchor: 0.5, yAnchor: 0.5, zIndex: 3
            });
        }
        miniMapDirOverlay.setPosition(position);
        miniMapDirOverlay.setMap(miniMap);
    }

    async function drawMiniMapPolygons(position: any) {
        miniMapPolygons.forEach(p => p.setMap(null));
        miniMapPolygons = [];

        const lat = position.getLat();
        const lng = position.getLng();
        const d = 0.003; // 대략 300m 안쪽
        const apiUrl = `${VITE_API_BASE_URL}/api/polygon/nearby?minLng=${lng - d}&minLat=${lat - d}&maxLng=${lng + d}&maxLat=${lat + d}`;
        try {
            const res = await fetch(apiUrl);
            if (!res.ok) return;
            const data = await res.json();
            data.forEach((item: any) => {
                const color = colorForSpecies(item.species);
                parseGeoJSON(item.geometry).forEach(path => {
                    if (path.length === 0) return;
                    const polygon = new kakao.maps.Polygon({
                        path, strokeWeight: 1, strokeColor: color.stroke, strokeOpacity: 0.9,
                        fillColor: color.fill, fillOpacity: 0.4
                    });
                    polygon.setMap(miniMap);
                    miniMapPolygons.push(polygon);
                });
            });
        } catch { /* 미니맵 폴리곤은 부가 정보라 실패해도 무시 */ }
    }

    function handleRoadviewMapClick(mouseEvent: any) {
        if (!roadviewClient) roadviewClient = new kakao.maps.RoadviewClient();
        const position = mouseEvent.latLng;
        roadviewClient.getNearestPanoId(position, 50, async (panoId: number | null) => {
            if (!panoId) return;
            roadviewOpen = true;
            await tick(); // 컨테이너가 실제로 화면에 표시된 뒤에 초기화/리레이아웃 해야 크기가 제대로 잡힘

            if (!roadviewInstance) {
                roadviewInstance = new kakao.maps.Roadview(roadviewContainer);
                kakao.maps.event.addListener(roadviewInstance, 'position_changed', () => {
                    const pos = roadviewInstance.getPosition();
                    if (miniMap) {
                        miniMap.setCenter(pos);
                        ensureMiniMapDirOverlay(pos);
                        drawMiniMapPolygons(pos);
                    }
                });
                kakao.maps.event.addListener(roadviewInstance, 'viewpoint_changed', () => {
                    const vp = roadviewInstance.getViewpoint();
                    if (miniMapDirEl) miniMapDirEl.style.transform = `rotate(${vp.pan}deg)`;
                });
            }
            roadviewInstance.setPanoId(panoId, position);
            roadviewInstance.relayout();

            if (!miniMap) {
                miniMap = new kakao.maps.Map(miniMapContainer, { center: position, level: 4 });
            } else {
                miniMap.setCenter(position);
                miniMap.relayout();
            }
            ensureMiniMapDirOverlay(position);
            drawMiniMapPolygons(position);
        });
    }

    let lastRoadviewMarkerEl: HTMLDivElement | null = null;
    let lastRoadviewMarkerOverlay: any;

    export function closeRoadview() {
        if (roadviewInstance) {
            const pos = roadviewInstance.getPosition();
            const vp = roadviewInstance.getViewpoint();
            if (pos) {
                if (!lastRoadviewMarkerEl) {
                    lastRoadviewMarkerEl = document.createElement('div');
                    lastRoadviewMarkerEl.style.cssText = 'pointer-events:none;transform-origin:50% 50%;';
                    lastRoadviewMarkerEl.innerHTML = `<svg width="34" height="34" viewBox="0 0 34 34">
                        <polygon points="17,3 6,30 17,23 28,30" fill="rgba(255,80,0,0.9)" stroke="white" stroke-width="1.5" stroke-linejoin="round"/>
                    </svg>`;
                    lastRoadviewMarkerOverlay = new kakao.maps.CustomOverlay({
                        position: pos, content: lastRoadviewMarkerEl, xAnchor: 0.5, yAnchor: 0.5, zIndex: 50
                    });
                }
                lastRoadviewMarkerEl.style.transform = `rotate(${vp.pan}deg)`;
                lastRoadviewMarkerOverlay.setPosition(pos);
                lastRoadviewMarkerOverlay.setMap(map);
                map.setCenter(pos);
            }
        }
        roadviewOpen = false;
    }

    export function getAddressFromCoords(lat: number, lng: number): Promise<string> {
        return new Promise((resolve) => {
            const geocoder = new kakao.maps.services.Geocoder();
            geocoder.coord2Address(lng, lat, (result: any, status: any) => {
                if (status === kakao.maps.services.Status.OK && result.length > 0) {
                    const addr = result[0];
                    resolve(addr.road_address?.address_name || addr.address?.address_name || '주소 없음');
                } else {
                    resolve('주소를 찾을 수 없습니다');
                }
            });
        });
    }

    export function search(query: string) {
        if (!ps) return;
        if (currentMarker) {
            currentMarker.setMap(null);
            currentMarker = null;
        }
        ps.keywordSearch(query, (data, status) => {
            if (status === kakao.maps.services.Status.OK) {
                dispatch('searchresults', { results: data });
                const bounds = new kakao.maps.LatLngBounds();
                bounds.extend(new kakao.maps.LatLng(data[0].y, data[0].x));
                map.setBounds(bounds);
                fetchAndDrawPolygons();
            } else {
                dispatch('searchresults', { results: [] });
            }
        });
    }

    export function setSavedLocations(locations: { id: number; name: string; lat: number; lng: number }[]) {
        savedLocationMarkers.forEach(m => m.setMap(null));
        savedLocationMarkers = [];
        if (!map) return;
        locations.forEach(loc => {
            const pos = new kakao.maps.LatLng(loc.lat, loc.lng);
            const marker = new kakao.maps.Marker({
                position: pos,
                map,
                title: loc.name,
                image: new kakao.maps.MarkerImage(
                    'https://t1.daumcdn.net/localimg/localimages/07/mapapidoc/markerStar.png',
                    new kakao.maps.Size(24, 35)
                )
            });
            const iw = new kakao.maps.InfoWindow({
                content: `<div style="padding:4px 8px;font-size:13px;white-space:nowrap;">${loc.name}</div>`,
                removable: true
            });
            kakao.maps.event.addListener(marker, 'click', () => {
                iw.open(map, marker);
            });
            savedLocationMarkers.push(marker);
        });
    }

    export function setCenter(y: number, x: number) {
        if (map) {
            const newPos = new kakao.maps.LatLng(y, x);
            map.setCenter(newPos);
            if (currentMarker) {
                currentMarker.setMap(null);
            }
            currentMarker = new kakao.maps.Marker({ position: newPos, map: map });
            fetchAndDrawPolygons();
        }
    }

    onMount(() => {
        configurePolygonCacheForDevice();
        // PC(769px 이상)에서는 범례를 기본으로 펼쳐서 보여줌 (모바일은 접힌 상태 + 스와이프 힌트 유지)
        if (window.matchMedia('(min-width: 769px)').matches) {
            legendExpanded = true;
        }

        const script = document.createElement('script');
        script.src = `//dapi.kakao.com/v2/maps/sdk.js?appkey=${VITE_KAKAO_MAP_API_KEY}&autoload=false&libraries=services`;
        script.async = true;
        script.onload = () => {
            if (typeof kakao !== 'undefined' && kakao.maps) {
                kakao.maps.load(() => {
                    initializeMap();
                    setupMapResizeHandling();
                });
            }
        };
        document.head.appendChild(script);
        return () => {
            fetchAbortController?.abort();
            if (nearestSpeciesMessageTimer) clearTimeout(nearestSpeciesMessageTimer);
            if (nearestHighlightTimer) clearTimeout(nearestHighlightTimer);
            if (overviewUpdateRaf !== null) cancelAnimationFrame(overviewUpdateRaf);
            hideActivePolygons();
            drawnPolygons.clear();
            polygonQueryCache = [];
            stopHeading();
            if (mapResizeObserver) mapResizeObserver.disconnect();
            if (interactionUnlockTimer) clearTimeout(interactionUnlockTimer);
        };
    });

</script>

<div bind:this={mapViewport} style="position:relative;width:100%;height:100vh;overflow:hidden;">
    <div bind:this={mapContainer} style="position:absolute;left:50%;top:50%;transform:rotate(0deg);"></div>

    {#if interactionShieldActive}
    <div
        role="presentation"
        on:pointerdown={handleInteractionShieldPointerDown}
        on:pointerup={handleInteractionShieldPointerEnd}
        on:pointercancel={handleInteractionShieldPointerEnd}
        on:wheel={handleInteractionShieldWheel}
        style="position:absolute;inset:0;z-index:4;background:transparent;touch-action:none;"
    ></div>
    {/if}

    {#if $locating}
    <div style="position:absolute;bottom:80px;left:50%;transform:translateX(-50%);z-index:200;pointer-events:none;
                display:flex;align-items:center;gap:8px;
                background:rgba(0,0,0,0.65);color:white;
                padding:8px 16px;border-radius:20px;font-size:13px;white-space:nowrap;">
        <svg width="16" height="16" viewBox="0 0 16 16" style="flex-shrink:0;animation:spin 1s linear infinite;">
            <circle cx="8" cy="8" r="6" fill="none" stroke="rgba(255,255,255,0.3)" stroke-width="2"/>
            <path d="M8 2 A6 6 0 0 1 14 8" fill="none" stroke="white" stroke-width="2" stroke-linecap="round"/>
        </svg>
        정확한 위치 찾는 중...
    </div>
    {/if}

    {#if controlsReady && !overviewHidden}
        <div class="overview-map {overviewExpanded ? 'expanded' : ''}">
            <button
                class="overview-map-canvas"
                on:click={() => overviewExpanded = !overviewExpanded}
                aria-label={overviewExpanded ? '대한민국 미니맵 축소' : '대한민국 미니맵 확대'}
                title={overviewExpanded ? '미니맵 축소' : '미니맵 확대'}
            >
                <svg viewBox="0 0 120 160" role="img" aria-label="현재 지도 위치를 표시한 대한민국 지도">
                    <defs>
                        <linearGradient id="overview-sea" x1="0" y1="0" x2="0" y2="1">
                            <stop offset="0" stop-color="#eaf5ff"/>
                            <stop offset="1" stop-color="#d8ecfa"/>
                        </linearGradient>
                        <filter id="overview-shadow" x="-20%" y="-20%" width="140%" height="140%">
                            <feDropShadow dx="0" dy="1" stdDeviation="1" flood-opacity="0.22"/>
                        </filter>
                    </defs>
                    <rect width="120" height="160" rx="9" fill="url(#overview-sea)"/>
                    <g stroke="#bad5e7" stroke-width="0.45" stroke-dasharray="2 3" opacity="0.65">
                        <path d="M6 42H114M6 80H114M6 118H114"/>
                        <path d="M33 6V154M60 6V154M87 6V154"/>
                    </g>
                    <g fill="#d7e7ce" stroke="#63855c" stroke-width="1.2" filter="url(#overview-shadow)">
                        <polygon points={KOREA_MAINLAND_POINTS}/>
                        <ellipse cx={JEJU_POINT.x} cy={JEJU_POINT.y} rx="7" ry="3.2" transform="rotate(-12 {JEJU_POINT.x} {JEJU_POINT.y})"/>
                        <circle cx={ULLEUNG_POINT.x} cy={ULLEUNG_POINT.y} r="1.8"/>
                    </g>
                    <path d="M31 14H89" stroke="#82999f" stroke-width="0.8" stroke-dasharray="2 2" opacity="0.8"/>
                    <rect
                        x={overviewViewport.x}
                        y={overviewViewport.y}
                        width={overviewViewport.width}
                        height={overviewViewport.height}
                        rx="1"
                        fill="rgba(26,115,232,0.18)"
                        stroke="#1a73e8"
                        stroke-width="1.4"
                    />
                    <circle cx={overviewCenterPoint.x} cy={overviewCenterPoint.y} r="4.8" fill="rgba(255,64,64,0.22)"/>
                    <circle cx={overviewCenterPoint.x} cy={overviewCenterPoint.y} r="2.5" fill="#e53935" stroke="white" stroke-width="1"/>
                    <text x="8" y="151" fill="#426071" font-size="7" font-weight="700">대한민국</text>
                </svg>
            </button>
            <button
                class="overview-map-close"
                on:click|stopPropagation={() => setOverviewHidden(true)}
                aria-label="대한민국 미니맵 숨기기"
                title="미니맵 숨기기"
            >×</button>
        </div>
    {:else if controlsReady && overviewHidden}
        <button
            class="overview-map-reopen"
            on:click={() => setOverviewHidden(false)}
            aria-label="대한민국 미니맵 보이기"
            title="미니맵 보이기"
        >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <path d="m3 6 5-3 8 3 5-3v15l-5 3-8-3-5 3V6Z"/><path d="M8 3v15M16 6v15"/>
            </svg>
        </button>
    {/if}

    <div
        role="group"
        aria-label="수종 범례"
        on:touchstart={legendSwipeStart}
        on:mousedown={legendSwipeStart}
        style="--legend-available-height:calc(100vh - {legendBottom}px - env(safe-area-inset-bottom, 0px) - 28px);
                --legend-available-height:calc(100dvh - {legendBottom}px - env(safe-area-inset-bottom, 0px) - 28px);
                position:fixed;bottom:calc({legendBottom}px + env(safe-area-inset-bottom, 0px));left:16px;z-index:150;
                display:flex;flex-direction:column;gap:2px;
                background:rgba(0,0,0,0.65);color:white;
                padding:6px 20px 6px 7px;border-radius:10px;font-size:12px;max-width:190px;
                touch-action:pan-y;user-select:none;cursor:grab;
                visibility:{controlsReady ? 'visible' : 'hidden'};
                transition:{controlsReady ? 'transform 0.25s ease, opacity 0.25s ease' : 'none'};
                transform:translateX({legendHidden ? '-150%' : '0'});
                opacity:{legendHidden ? 0 : 1};
                pointer-events:{legendHidden ? 'none' : 'auto'};">
        <button
            class="legend-swipe-handle"
            on:click|stopPropagation={() => setLegendHidden(true)}
            aria-label="왼쪽 메뉴 숨기기"
            title="왼쪽 메뉴 숨기기"
        ></button>
        <div class="legend-species-list">
        {#each speciesLegendPrimary as item}
        <div class="legend-species-row {selectedSpecies === item.name ? 'selected' : ''} {selectedSpecies && selectedSpecies !== item.name ? 'muted' : ''}">
            <button class="legend-species-select" on:click={() => toggleSpeciesHighlight(item.name)} title="{item.name} 강조" aria-pressed={selectedSpecies === item.name}>
                <span class="legend-color" style="background:{item.color};"></span>
                <span>{item.name}</span>
            </button>
            <button class="legend-nearest-button" on:click={() => moveToNearestSpecies(item.name)} title="가장 가까운 {item.name}로 이동" aria-label="가장 가까운 {item.name}로 이동">
                {#if nearestSpeciesLoading === item.name}
                    <span class="legend-nearest-spinner" aria-hidden="true"></span>
                {:else}
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                        <circle cx="12" cy="12" r="3"/><path d="M12 2v3M12 19v3M2 12h3M19 12h3"/>
                    </svg>
                {/if}
            </button>
        </div>
        {/each}
        {#if legendExpanded}
        {#each speciesLegendMore as item}
        <div class="legend-species-row secondary {selectedSpecies === item.name ? 'selected' : ''} {selectedSpecies && selectedSpecies !== item.name ? 'muted' : ''}">
            <button class="legend-species-select" on:click={() => toggleSpeciesHighlight(item.name)} title="{item.name} 강조" aria-pressed={selectedSpecies === item.name}>
                <span class="legend-color" style="background:{item.color};"></span>
                <span>{item.name}</span>
            </button>
            <button class="legend-nearest-button" on:click={() => moveToNearestSpecies(item.name)} title="가장 가까운 {item.name}로 이동" aria-label="가장 가까운 {item.name}로 이동">
                {#if nearestSpeciesLoading === item.name}
                    <span class="legend-nearest-spinner" aria-hidden="true"></span>
                {:else}
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                        <circle cx="12" cy="12" r="3"/><path d="M12 2v3M12 19v3M2 12h3M19 12h3"/>
                    </svg>
                {/if}
            </button>
        </div>
        {/each}
        {/if}
        </div>
        <button
            class="legend-expand-button"
            on:click={() => legendExpanded = !legendExpanded}
        >{legendExpanded ? '접기 ▲' : '· · · 더보기'}</button>
    </div>

    {#if controlsReady && legendHidden}
    <button
        on:touchstart={legendSwipeStart}
        on:mousedown={legendSwipeStart}
        on:click={() => setLegendHidden(false)}
        aria-label="왼쪽 메뉴 보이기"
        title="왼쪽 메뉴 보이기"
        style="position:fixed;top:52%;left:0;transform:translateY(-50%);z-index:161;
                width:22px;height:64px;padding:0;border:0;border-radius:0 10px 10px 0;
                background:rgba(0,0,0,0.62);color:white;font-size:13px;
                box-shadow:2px 0 8px rgba(0,0,0,0.2);
                touch-action:pan-y;cursor:pointer;">▶</button>
    {/if}

    {#if nearestSpeciesMessage}
        <div class="nearest-species-toast" role="status">{nearestSpeciesMessage}</div>
    {/if}

    <div style="position:fixed;inset:0;z-index:400;display:{roadviewOpen ? 'block' : 'none'};background:#000;">
        <div bind:this={roadviewContainer} style="width:100%;height:100%;"></div>
        <div bind:this={miniMapContainer} style="position:absolute;top:16px;left:16px;z-index:401;
                   width:150px;height:150px;border-radius:10px;overflow:hidden;
                   box-shadow:0 2px 12px rgba(0,0,0,0.5);border:2px solid rgba(255,255,255,0.8);"></div>
        <button
            on:click={closeRoadview}
            style="position:absolute;top:16px;right:16px;z-index:401;
                   width:40px;height:40px;border-radius:50%;border:none;
                   background:rgba(0,0,0,0.6);color:white;font-size:18px;
                   display:flex;align-items:center;justify-content:center;cursor:pointer;"
            title="거리뷰 닫기"
        >✕</button>
    </div>
</div>

<style>
    @keyframes spin {
        from { transform: rotate(0deg); }
        to   { transform: rotate(360deg); }
    }

    .overview-map {
        position: absolute;
        top: 84px;
        left: 16px;
        z-index: 9;
        width: 102px;
        height: 136px;
        border: 1px solid rgba(255, 255, 255, 0.9);
        border-radius: 11px;
        background: #eaf5ff;
        box-shadow: 0 3px 12px rgba(0, 0, 0, 0.24);
        overflow: hidden;
        transition: width 0.2s ease, height 0.2s ease, opacity 0.2s ease;
    }
    .overview-map.expanded {
        width: 162px;
        height: 216px;
    }
    .overview-map-canvas {
        width: 100%;
        height: 100%;
        display: block;
        padding: 0;
        border: 0;
        background: transparent;
        cursor: pointer;
    }
    .overview-map-canvas svg {
        width: 100%;
        height: 100%;
        display: block;
    }
    .overview-map-close {
        position: absolute;
        top: 4px;
        right: 4px;
        width: 22px;
        height: 22px;
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 0 0 2px;
        border: 0;
        border-radius: 50%;
        background: rgba(25, 45, 58, 0.68);
        color: white;
        font-size: 17px;
        line-height: 1;
        cursor: pointer;
    }
    .overview-map-reopen {
        position: absolute;
        top: 96px;
        left: 0;
        z-index: 9;
        width: 30px;
        height: 48px;
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 5px;
        border: 0;
        border-radius: 0 9px 9px 0;
        background: rgba(25, 45, 58, 0.72);
        color: white;
        box-shadow: 2px 2px 9px rgba(0, 0, 0, 0.2);
        cursor: pointer;
    }
    .overview-map-reopen svg {
        width: 19px;
        height: 19px;
    }

    @media (max-width: 768px) {
        .overview-map {
            top: 100px;
            left: 10px;
            width: 88px;
            height: 118px;
        }
        .overview-map.expanded {
            width: 148px;
            height: 198px;
        }
        .overview-map-reopen {
            top: 108px;
        }
    }

    .legend-swipe-handle {
        position: absolute;
        right: -2px;
        top: 50%;
        width: 20px;
        height: 56px;
        padding: 0;
        border: 0;
        background: transparent;
        display: flex;
        align-items: center;
        justify-content: center;
        cursor: pointer;
        transform: translateY(-50%);
        animation: legend-handle-nudge 2.2s ease-in-out infinite;
    }

    .legend-species-row {
        display: flex;
        align-items: center;
        min-height: 25px;
        border-radius: 6px;
        transition: background 0.18s ease, opacity 0.18s ease;
    }
    .legend-species-row.secondary {
        opacity: 0.86;
    }
    .legend-species-row.selected {
        background: rgba(77, 171, 247, 0.3);
        opacity: 1;
    }
    .legend-species-row.muted {
        opacity: 0.5;
    }
    .legend-species-select {
        min-width: 0;
        flex: 1;
        display: flex;
        align-items: center;
        gap: 6px;
        padding: 2px 4px;
        border: 0;
        background: transparent;
        color: inherit;
        font: inherit;
        text-align: left;
        white-space: nowrap;
        cursor: pointer;
    }
    .legend-color {
        width: 10px;
        height: 10px;
        border-radius: 2px;
        flex-shrink: 0;
    }
    .legend-nearest-button {
        width: 25px;
        height: 25px;
        flex: 0 0 25px;
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 5px;
        border: 0;
        border-radius: 50%;
        background: transparent;
        color: #8dccff;
        cursor: pointer;
    }
    .legend-nearest-button:active {
        background: rgba(255, 255, 255, 0.18);
    }
    .legend-nearest-button svg {
        width: 16px;
        height: 16px;
    }
    .legend-species-list {
        display: flex;
        flex-direction: column;
        gap: 1px;
        max-height: min(42vh, var(--legend-available-height));
        max-height: min(42dvh, var(--legend-available-height));
        padding-right: 2px;
        overflow-x: hidden;
        overflow-y: auto;
        overscroll-behavior: contain;
        scrollbar-width: thin;
        scrollbar-color: rgba(141, 204, 255, 0.75) rgba(255, 255, 255, 0.08);
    }
    .legend-species-list::-webkit-scrollbar {
        width: 4px;
    }
    .legend-species-list::-webkit-scrollbar-track {
        background: rgba(255, 255, 255, 0.08);
        border-radius: 2px;
    }
    .legend-species-list::-webkit-scrollbar-thumb {
        background: rgba(141, 204, 255, 0.75);
        border-radius: 2px;
    }
    .legend-expand-button {
        margin-top: 1px;
        padding: 2px 4px;
        border: 0;
        background: none;
        color: rgba(255, 255, 255, 0.75);
        font-size: 11px;
        cursor: pointer;
        text-align: left;
    }
    .legend-nearest-spinner {
        width: 13px;
        height: 13px;
        border: 2px solid rgba(255, 255, 255, 0.3);
        border-top-color: #8dccff;
        border-radius: 50%;
        animation: spin 0.8s linear infinite;
    }
    .nearest-species-toast {
        position: fixed;
        left: 50%;
        bottom: calc(82px + env(safe-area-inset-bottom, 0px));
        z-index: 220;
        transform: translateX(-50%);
        padding: 9px 15px;
        border-radius: 20px;
        background: rgba(20, 24, 28, 0.86);
        color: white;
        box-shadow: 0 3px 12px rgba(0, 0, 0, 0.25);
        font-size: 13px;
        white-space: nowrap;
        pointer-events: none;
    }
    .legend-swipe-handle::before {
        content: '';
        width: 4px;
        height: 36px;
        border-radius: 2px;
        background: #4dabf7;
        box-shadow: 0 0 6px rgba(77, 171, 247, 0.65);
    }

    @keyframes legend-handle-nudge {
        0%, 20%, 100% { transform: translateY(-50%) translateX(0); opacity: 0.6; }
        10% { transform: translateY(-50%) translateX(-5px); opacity: 1; }
    }

    /* PC에서는 스와이프 안내 애니메이션이 불필요하니 정지 */
    @media (min-width: 769px) {
        .legend-swipe-handle {
            animation: none;
            opacity: 0.4;
        }
    }
</style>
