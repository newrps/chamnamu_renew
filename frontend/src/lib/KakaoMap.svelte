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
    let mapContainer: HTMLElement;
    let currentMarker: kakao.maps.Marker | null = null;
    export let legendBottom: number = 136;

    let drawnPolygons = new Map<string, kakao.maps.Polygon[]>();
    let polygonsFetchedOnce = false;

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
    export const speciesLegendPrimary = OAK_SPECIES.map(name => ({ name, color: SPECIES_COLORS[name].fill }));
    export const speciesLegendMore = OTHER_SPECIES.map(name => ({ name, color: SPECIES_COLORS[name].fill }));
    let legendExpanded = false;

    // 범례 좌우 스와이프로 숨기기/보이기
    let legendHidden = false;
    let legendSwipeStartX = 0;
    let legendSwiping = false;

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
            legendHidden = true;
        } else if (deltaX > 40 && legendHidden) {
            legendHidden = false;
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

    let isHeadingActive = false;
    // 추적 중 지도를 직접 드래그하면 재중심/회전을 멈추고(다음지도 방식), 나침반 버튼으로 다시 재중심함
    let isFollowing = true;
    let dragStopHeadingTimer: ReturnType<typeof setTimeout> | null = null;
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

    let appliedRotationScale = 1; // mapContainer에 실제로 적용된 확대 배율 (드래그 보정 계산에 사용)
    // 임시 디버그 표시용 - 드래그/줌 보정 버그를 실제 기기에서 확인하기 위한 값들
    let debugScreenDx = 0, debugScreenDy = 0, debugInternalDx = 0, debugInternalDy = 0;
    let debugPinchRatio = 1, debugPinchTargetLevel = 0;
    let currentHeading = 0;
    let continuousHeading = 0;
    let appliedRotationAngle = 0; // mapContainer에 실제로 적용된 CSS 회전각 (드래그 보정 계산에 사용)
    let rafId: number | null = null;

    // 회전된 사각형(지도 컨테이너)이 원래 화면 영역을 모서리까지 완전히 덮으려면 필요한 최소 확대 배율.
    // 화면이 정사각형이 아니라서(특히 세로가 긴 모바일) 고정 배율로는 45도 부근에서 모서리가 비어 보였음
    function coverageScale(angleDeg: number): number {
        if (!mapContainer) return 1;
        const w = mapContainer.clientWidth || 1;
        const h = mapContainer.clientHeight || 1;
        const rad = angleDeg * Math.PI / 180;
        const cos = Math.abs(Math.cos(rad));
        const sin = Math.abs(Math.sin(rad));
        const a = w / 2, b = h / 2;
        const needed = Math.max(cos + (b / a) * sin, (a / b) * sin + cos);
        return Math.min(needed * 1.05, 3); // 여유 5% + 과도한 확대 방지 상한
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

    // 핀치줌 도중 정수 레벨 사이를 부드럽게 이어 보이도록 얹는 임시 CSS 배율 (제스처 끝나면 1로 리셋)
    let pinchLiveScale = 1;

    function applyContainerTransform(angle: number, withTransition: boolean) {
        if (!mapContainer) return;
        const scale = coverageScale(angle) * pinchLiveScale;
        mapContainer.style.transition = withTransition ? 'transform 0.2s linear' : 'none';
        mapContainer.style.transform = `rotate(${angle}deg) scale(${scale})`;
        mapContainer.style.transformOrigin = '50% 50%';
        appliedRotationAngle = angle;
        appliedRotationScale = scale;
    }

    function applyMapRotation(heading: number) {
        currentHeading = heading;
        const current = ((continuousHeading % 360) + 360) % 360;
        let diff = heading - current;
        if (diff > 180) diff -= 360;
        if (diff < -180) diff += 360;
        continuousHeading += diff;

        // 재중심(팔로잉) 여부와 상관없이 회전 자체는 항상 실시간으로 반영 - 화면을 옮겨서 보고 있어도 방향은 계속 맞아야 함.
        // 단, 드래그/핀치 제스처 도중엔 화면 회전을 잠깐 멈춤 - 손으로 폰을 쥐고 움직이는 동안
        // 나침반 값이 미세하게 흔들려서 드래그 중간에 회전이 계속 바뀌면 궤적이 휘어져 보이는 문제가 있었음
        if (customDragActive || pinchActive) return;
        if (rafId !== null) cancelAnimationFrame(rafId);
        const angle = continuousHeading;
        rafId = requestAnimationFrame(() => {
            if (overlayElement) {
                overlayElement.style.transition = 'transform 0.2s linear';
                overlayElement.style.transform = `rotate(${-angle}deg)`;
            }
            applyContainerTransform(angle, true);
            rafId = null;
        });
    }

    function createOrUpdateLocationOverlay(lat: number, lng: number) {
        const pos = new kakao.maps.LatLng(lat, lng);
        if (!locationOverlay) {
            const el = document.createElement('div');
            el.style.cssText = 'pointer-events:none;transform-origin:50% 75%;';
            el.innerHTML = `<svg width="36" height="47" viewBox="0 0 56 72" xmlns="http://www.w3.org/2000/svg">
                <polygon points="28,46 2,2 54,2" fill="rgba(66,133,244,0.35)" stroke="rgba(66,133,244,0.5)" stroke-width="1" stroke-linejoin="round"/>
                <circle cx="28" cy="54" r="15" fill="white" style="filter:drop-shadow(0 2px 6px rgba(0,0,0,0.35))"/>
                <circle cx="28" cy="54" r="11" fill="#4285f4"/>
                <circle cx="28" cy="54" r="4" fill="white"/>
            </svg>`;
            overlayElement = el;
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
            if (event.alpha === null) return;
            absoluteOrientationReceived = true;
            const heading = event.alpha;
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
            let heading: number;
            if (event.webkitCompassHeading !== undefined) {
                heading = event.webkitCompassHeading;
            } else if (event.alpha !== null && event.alpha !== undefined) {
                heading = event.alpha;
            } else {
                return;
            }
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

    export function startHeading() {
        if (!navigator.geolocation) {
            alert('이 브라우저에서는 위치 서비스를 지원하지 않습니다.');
            return;
        }

        if (isHeadingActive) {
            stopHeading();
            return;
        }

        isHeadingActive = true;
        isFollowing = true;
        // 지도가 CSS로 회전되어 있으면 카카오 기본 드래그/줌(회전을 모르고 화면 좌표 그대로 계산)이
        // 어긋나서(드래그 방향이 틀리거나 줌이 엉뚱한 곳으로 튐) 직접 처리함
        if (map) {
            (map as any).setDraggable(false);
            (map as any).setZoomable(false);
        }
        startOrientationTracking();
        locating.set(true);

        navigator.geolocation.getCurrentPosition(
            (position) => {
                const lat = position.coords.latitude;
                const lng = position.coords.longitude;
                currentLat = lat;
                currentLng = lng;
                panToPosition(lat, lng);
            },
            () => {},
            { enableHighAccuracy: false, maximumAge: 30000, timeout: 3000 }
        );

        let gpsFixed = false;
        watchId = navigator.geolocation.watchPosition(
            (position) => {
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
                    updatePolygonsIfNeeded(lat, lng);
                }

                const gpsHeading = position.coords.heading;
                if (!absoluteOrientationReceived && gpsHeading !== null && gpsHeading !== undefined) {
                    currentHeading = gpsHeading;
                    applyMapRotation(gpsHeading);
                }

                dispatch('headingupdate', { lat, lng, heading: currentHeading, accuracy: position.coords.accuracy });
            },
            (error) => {
                console.error('위치 정보를 가져오는데 실패했습니다:', error);
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

    export function stopHeading() {
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
        appliedRotationScale = 1;
        pinchLiveScale = 1;
        absoluteOrientationReceived = false;
        if (mapContainer) {
            mapContainer.style.transition = '';
            mapContainer.style.transform = '';
        }
        if (map) {
            (map as any).setDraggable(true);
            (map as any).setZoomable(true);
        }
        pinchActive = false;
        lastPolygonUpdate = 0;
        lastCenter = null;

        dispatch('headingstop');
    }

    // 지도가 CSS로 회전된 상태에서는 카카오 기본 드래그/줌(회전을 모르고 항상 북쪽 기준 화면 좌표로 계산)이
    // 화면에 보이는 방향과 어긋나므로, 추적 중엔 기본 드래그/줌을 끄고(setDraggable/setZoomable(false)) 직접 처리함
    const MIN_ZOOM_LEVEL = 1;
    const MAX_ZOOM_LEVEL = 7;
    let customDragActive = false;
    let customDragLastX = 0;
    let customDragLastY = 0;
    let pinchActive = false;
    let pinchStartDist = 0;
    let pinchStartLevel = 3;

    // panBy는 부드럽게(애니메이션으로) 움직이는 함수라 연속으로 자주 호출하면
    // 매번 애니메이션이 도중에 끊기면서 실제로는 아주 조금만 움직인 것처럼 보임 -
    // 화면 좌표 <-> 지도 좌표 변환(Projection)으로 직접 setCenter해서 즉시 이동시킴
    function panByRotated(dx: number, dy: number) {
        if (!map) return;
        isProgrammaticPan = true;
        const proj = (map as any).getProjection();
        const centerPoint = proj.containerPointFromCoords(map.getCenter());
        const newPoint = new kakao.maps.Point(centerPoint.x + dx, centerPoint.y + dy);
        const newCenter = proj.coordsFromContainerPoint(newPoint);
        map.setCenter(newCenter);
        if (programmaticPanTimer) clearTimeout(programmaticPanTimer);
        programmaticPanTimer = setTimeout(() => {
            isProgrammaticPan = false;
        }, 600);
    }

    // 드래그 제스처 도중엔 회전각을 고정해서 사용 - 회전은 계속 실시간으로 반영되는데(헤딩 중),
    // 폰을 손에 쥐고 드래그하는 동안 실제로 기기 방향(나침반)이 미세하게 계속 흔들려서
    // 위/아래로 왔다갔다 할 때마다 매번 다른 각도로 보정되어 한쪽으로 계속 밀리는(누적 오차) 문제가 있었음
    let dragRefAngle = 0;
    let dragRefScale = 1;

    // 화면(회전된 상태)에서 느낀 이동량을, 지도 내부의 회전 안 된 좌표계 기준 이동량으로 역변환
    function rotatedPanDelta(screenDx: number, screenDy: number) {
        const rad = dragRefAngle * Math.PI / 180;
        const cos = Math.cos(rad);
        const sin = Math.sin(rad);
        return {
            x: screenDx * cos + screenDy * sin,
            y: -screenDx * sin + screenDy * cos
        };
    }

    function customDragStart(clientX: number, clientY: number) {
        if (!isHeadingActive) return;
        customDragActive = true;
        customDragLastX = clientX;
        customDragLastY = clientY;
        dragRefAngle = appliedRotationAngle;
        dragRefScale = appliedRotationScale;
        if (isFollowing) pauseFollowing();
    }

    function customDragMove(clientX: number, clientY: number) {
        if (!customDragActive) return;
        const screenDx = clientX - customDragLastX;
        const screenDy = clientY - customDragLastY;
        customDragLastX = clientX;
        customDragLastY = clientY;
        const delta = rotatedPanDelta(screenDx, screenDy);
        // 지도 컨테이너가 확대돼 있어서, 화면상 이동량을 지도 내부 픽셀 단위로 환산
        const internalDx = delta.x / dragRefScale;
        const internalDy = delta.y / dragRefScale;
        debugScreenDx = screenDx; debugScreenDy = screenDy;
        debugInternalDx = internalDx; debugInternalDy = internalDy;
        // 드래그 방향으로 화면 내용이 손가락을 따라오도록 부호 반전
        panByRotated(-internalDx, -internalDy);
    }

    function customDragEnd() {
        if (!customDragActive) return;
        customDragActive = false;
        if (isHeadingActive) applyMapRotation(currentHeading); // 드래그 중 멈춰뒀던 회전을 최신값으로 즉시 반영
        fetchAndDrawPolygons();
    }

    function handleContainerMouseDown(e: MouseEvent) {
        if (!isHeadingActive) return;
        customDragStart(e.clientX, e.clientY);
        window.addEventListener('mousemove', handleWindowMouseMove);
        window.addEventListener('mouseup', handleWindowMouseUp, { once: true });
    }
    function handleWindowMouseMove(e: MouseEvent) {
        customDragMove(e.clientX, e.clientY);
    }
    function handleWindowMouseUp() {
        customDragEnd();
        window.removeEventListener('mousemove', handleWindowMouseMove);
    }

    // 회전 중엔 카카오 기본 줌(핀치/휠)이 어디를 기준으로 확대할지 화면 좌표로 계산하다가
    // 회전된 화면과 어긋나서 엉뚱한 곳으로 튀는 문제가 있었음 - anchor 없이 setLevel만 호출해서
    // 항상 "현재 지도 중심" 기준으로 확대/축소함 (회전 중심=화면 중심이라 항상 안전함)
    function touchDistance(t0: Touch, t1: Touch) {
        const dx = t1.clientX - t0.clientX;
        const dy = t1.clientY - t0.clientY;
        return Math.sqrt(dx * dx + dy * dy);
    }

    function startPinch(t0: Touch, t1: Touch) {
        if (!map) return;
        pinchActive = true;
        pinchStartDist = touchDistance(t0, t1);
        pinchStartLevel = map.getLevel();
    }

    function updatePinch(t0: Touch, t1: Touch) {
        if (!map || pinchStartDist <= 0) return;
        const dist = touchDistance(t0, t1);
        const ratio = dist / pinchStartDist;
        // 카카오 레벨은 낮을수록 확대(줌인) - 손가락을 벌리면(ratio>1) 레벨을 낮춤. 정수 레벨이 아니라
        // 연속값(rawTarget)으로 계산해서, 실제 레벨(정수)로 스냅하기 전까지의 차이를 CSS 확대로 보여줌
        // -> 카카오 레벨 전환이 매번 뚝뚝 끊기지 않고 부드럽게 이어져 보임
        const rawTarget = pinchStartLevel - Math.log2(ratio);
        let targetLevel = Math.round(rawTarget);
        targetLevel = Math.max(MIN_ZOOM_LEVEL, Math.min(MAX_ZOOM_LEVEL, targetLevel));
        if (targetLevel !== map.getLevel()) {
            map.setLevel(targetLevel);
        }
        const committedLevel = map.getLevel();
        pinchLiveScale = Math.pow(2, committedLevel - rawTarget);
        applyContainerTransform(appliedRotationAngle, false);
        debugPinchRatio = ratio;
        debugPinchTargetLevel = targetLevel;
    }

    function endPinch() {
        if (!pinchActive) return;
        pinchActive = false;
        pinchStartDist = 0;
        pinchLiveScale = 1;
        if (isHeadingActive) {
            applyMapRotation(currentHeading); // 핀치 중 멈춰뒀던 회전을 최신값으로 즉시 반영
        } else {
            applyContainerTransform(appliedRotationAngle, true);
        }
        fetchAndDrawPolygons();
    }

    function handleContainerWheel(e: WheelEvent) {
        if (!isHeadingActive || !map) return;
        e.preventDefault();
        const level = map.getLevel();
        const newLevel = Math.max(MIN_ZOOM_LEVEL, Math.min(MAX_ZOOM_LEVEL, level + (e.deltaY > 0 ? 1 : -1)));
        if (newLevel !== level) map.setLevel(newLevel);
    }

    function handleContainerTouchStart(e: TouchEvent) {
        if (!isHeadingActive) return;
        if (e.touches.length === 2) {
            startPinch(e.touches[0], e.touches[1]);
            window.addEventListener('touchmove', handleWindowTouchMove, { passive: false });
            window.addEventListener('touchend', handleWindowTouchEnd);
            window.addEventListener('touchcancel', handleWindowTouchEnd);
            return;
        }
        if (e.touches.length !== 1) return;
        customDragStart(e.touches[0].clientX, e.touches[0].clientY);
        window.addEventListener('touchmove', handleWindowTouchMove, { passive: false });
        window.addEventListener('touchend', handleWindowTouchEnd);
        window.addEventListener('touchcancel', handleWindowTouchEnd);
    }
    function handleWindowTouchMove(e: TouchEvent) {
        if (e.touches.length === 2) {
            if (customDragActive) customDragActive = false; // 드래그 중 손가락이 늘면 핀치로 전환
            if (!pinchActive) startPinch(e.touches[0], e.touches[1]);
            e.preventDefault();
            updatePinch(e.touches[0], e.touches[1]);
            return;
        }
        if (e.touches.length === 1) {
            if (pinchActive) {
                // 핀치 중 손가락 하나를 떼면 남은 손가락으로 드래그 재개
                endPinch();
                customDragStart(e.touches[0].clientX, e.touches[0].clientY);
            }
            if (!customDragActive) return;
            e.preventDefault();
            customDragMove(e.touches[0].clientX, e.touches[0].clientY);
        }
    }
    function handleWindowTouchEnd(e: TouchEvent) {
        if (e.touches.length === 1) {
            endPinch();
            customDragStart(e.touches[0].clientX, e.touches[0].clientY);
            return;
        }
        if (e.touches.length > 0) return;
        customDragEnd();
        endPinch();
        window.removeEventListener('touchmove', handleWindowTouchMove);
        window.removeEventListener('touchend', handleWindowTouchEnd);
        window.removeEventListener('touchcancel', handleWindowTouchEnd);
    }

    function initializeMap() {
        if (typeof kakao !== 'undefined' && kakao.maps) {
            const mapOption = {
                center: new kakao.maps.LatLng(37.254971339188, 127.1148388815),
                level: 3,
                maxLevel: 7
            };
            map = new kakao.maps.Map(mapContainer, mapOption);
            ps = new kakao.maps.services.Places();

            fetchAndDrawPolygons();

            mapContainer.addEventListener('mousedown', handleContainerMouseDown);
            mapContainer.addEventListener('touchstart', handleContainerTouchStart, { passive: true });
            mapContainer.addEventListener('wheel', handleContainerWheel, { passive: false });

            kakao.maps.event.addListener(map, 'dragend', fetchAndDrawPolygons);
            kakao.maps.event.addListener(map, 'dragstart', () => {
                // 우리 코드가 panTo()로 지도를 움직인 것이면(현재위치 추적 중 계속 발생) 무시
                if (isProgrammaticPan) return;
                // 추적 중이 아니거나 이미 일시정지 상태면 할 일 없음
                if (!isHeadingActive || !isFollowing) return;
                // 모바일 핀치줌 제스처가 시작될 때도 dragstart가 같이 발생해서, 줌인지 실제 드래그인지
                // 잠깐 기다렸다가 판단함 (그 사이 zoom_changed가 오면 줌으로 간주하고 취소)
                if (dragStopHeadingTimer) clearTimeout(dragStopHeadingTimer);
                dragStopHeadingTimer = setTimeout(() => {
                    dragStopHeadingTimer = null;
                    pauseFollowing();
                }, 150);
            });
            kakao.maps.event.addListener(map, 'zoom_changed', () => {
                if (dragStopHeadingTimer) {
                    clearTimeout(dragStopHeadingTimer);
                    dragStopHeadingTimer = null;
                }
                fetchAndDrawPolygons();
            });
            kakao.maps.event.addListener(map, 'click', hidePolygonInfo);
        } else {
            console.error("카카오맵 API 스크립트가 아직 로드되지 않았습니다.");
        }
    }

    let mapResizeObserver: ResizeObserver | null = null;

    // 지도 컨테이너 크기가 바뀔 때(창 크기 조절, 광고 배너 표시/숨김 등) 카카오맵이 자동으로
    // 알아채지 못해서 현재 위치 오버레이 등이 어긋나 보일 수 있음 - relayout으로 강제 재계산
    function setupMapResizeHandling() {
        if (!mapContainer || !map) return;
        mapResizeObserver = new ResizeObserver(() => {
            // 나침반/현재위치 추적 중엔 건드리지 않음 - relayout/setCenter가 추적을 끊어버리는 문제가 있었음
            if (!map || isHeadingActive) return;
            (map as any).relayout();
        });
        mapResizeObserver.observe(mapContainer);
    }

    function getDistance(lat1: number, lng1: number, lat2: number, lng2: number): number {
        const dLat = lat1 - lat2;
        const dLng = lng1 - lng2;
        return Math.sqrt(dLat * dLat + dLng * dLng);
    }

    // 뷰포트가 이 정도(위/경도 기준)보다 넓게 확대축소되면 서버에서도 빈 목록을 주므로
    // 굳이 요청을 안 보내고 화면 폴리곤만 정리한다 (백엔드 MAX_BBOX_DEGREES와 맞출 것)
    const MAX_BBOX_DEGREES = 1.0;

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

    async function fetchAndDrawPolygons() {
        if (!map) return;

        if (fetchAbortController) fetchAbortController.abort();
        fetchAbortController = new AbortController();
        const thisController = fetchAbortController;
        const signal = thisController.signal;

        const { minLat, minLng, maxLat, maxLng } = getPaddedBounds();

        if ((maxLng - minLng) > MAX_BBOX_DEGREES || (maxLat - minLat) > MAX_BBOX_DEGREES) {
            // 너무 축소된 상태 - 전국 단위 조회는 무거우니 건너뛰고 기존 폴리곤만 정리
            for (const [key, polys] of drawnPolygons) {
                polys.forEach(p => p.setMap(null));
            }
            drawnPolygons.clear();
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

            data.forEach((item: any) => {
                const key = String(item.id);
                incomingIds.add(key);
                if (drawnPolygons.has(key)) return;
                const paths = parseGeoJSON(item.geometry);
                const color = colorForSpecies(item.species);
                const newPolys: kakao.maps.Polygon[] = [];
                paths.forEach((path: kakao.maps.LatLng[]) => {
                    if (path.length > 0) {
                        const polygon = new kakao.maps.Polygon({
                            path,
                            strokeWeight: 1,
                            strokeColor: color.stroke,
                            strokeOpacity: 0.9,
                            fillColor: color.fill,
                            fillOpacity: 0.4
                        });
                        polygon.setMap(map);
                        kakao.maps.event.addListener(polygon, 'mouseover', (e: any) => {
                            polygon.setOptions({ fillOpacity: 0.8, strokeWeight: 2 });
                            showPolygonInfo(item.species, e.latLng);
                        });
                        kakao.maps.event.addListener(polygon, 'mousemove', (e: any) => {
                            if (polygonInfoOverlay) polygonInfoOverlay.setPosition(e.latLng);
                        });
                        kakao.maps.event.addListener(polygon, 'mouseout', () => {
                            polygon.setOptions({ fillOpacity: 0.4, strokeWeight: 1 });
                            hidePolygonInfo();
                        });
                        let tapHighlightTimeout: ReturnType<typeof setTimeout> | null = null;
                        kakao.maps.event.addListener(polygon, 'click', (e: any) => {
                            kakao.maps.event.preventMap();
                            polygon.setOptions({ fillOpacity: 0.8, strokeWeight: 2 });
                            showPolygonInfo(item.species, e.latLng);
                            if (tapHighlightTimeout) clearTimeout(tapHighlightTimeout);
                            tapHighlightTimeout = setTimeout(() => {
                                polygon.setOptions({ fillOpacity: 0.4, strokeWeight: 1 });
                                hidePolygonInfo();
                            }, 2000);
                        });
                        newPolys.push(polygon);
                    }
                });
                drawnPolygons.set(key, newPolys);
            });

            for (const [key, polys] of drawnPolygons) {
                if (!incomingIds.has(key)) {
                    polys.forEach(p => p.setMap(null));
                    drawnPolygons.delete(key);
                }
            }

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
            stopHeading();
            if (mapResizeObserver) mapResizeObserver.disconnect();
            if (mapContainer) {
                mapContainer.removeEventListener('mousedown', handleContainerMouseDown);
                mapContainer.removeEventListener('touchstart', handleContainerTouchStart);
                mapContainer.removeEventListener('wheel', handleContainerWheel);
            }
            window.removeEventListener('mousemove', handleWindowMouseMove);
            window.removeEventListener('touchmove', handleWindowTouchMove);
            window.removeEventListener('touchend', handleWindowTouchEnd);
            window.removeEventListener('touchcancel', handleWindowTouchEnd);
        };
    });

</script>

<div style="position:relative;width:100%;height:100vh;overflow:hidden;">
    <div bind:this={mapContainer} style="width:100%;height:100%;"></div>

    {#if isHeadingActive}
    <div style="position:absolute;top:8px;left:8px;z-index:500;pointer-events:none;
        background:rgba(0,0,0,0.75);color:#0f0;font-family:monospace;font-size:11px;
        line-height:1.5;padding:6px 8px;border-radius:6px;white-space:pre;">
angle={appliedRotationAngle.toFixed(1)} scale={appliedRotationScale.toFixed(2)}
drag screen=({debugScreenDx.toFixed(0)},{debugScreenDy.toFixed(0)}) internal=({debugInternalDx.toFixed(1)},{debugInternalDy.toFixed(1)})
pinch ratio={debugPinchRatio.toFixed(2)} targetLevel={debugPinchTargetLevel}
    </div>
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

    <div
        on:touchstart={legendSwipeStart}
        on:mousedown={legendSwipeStart}
        style="position:fixed;bottom:calc({legendBottom}px + env(safe-area-inset-bottom, 0px));left:16px;z-index:150;
                display:flex;flex-direction:column;gap:4px;
                background:rgba(0,0,0,0.65);color:white;
                padding:8px 20px 8px 12px;border-radius:10px;font-size:12px;max-width:150px;
                touch-action:pan-y;user-select:none;cursor:grab;
                transition:transform 0.25s ease, opacity 0.25s ease;
                transform:translateX({legendHidden ? '-150%' : '0'});
                opacity:{legendHidden ? 0 : 1};
                pointer-events:{legendHidden ? 'none' : 'auto'};">
        <span class="legend-swipe-handle"></span>
        {#each speciesLegendPrimary as item}
        <div style="display:flex;align-items:center;gap:6px;white-space:nowrap;">
            <span style="width:10px;height:10px;border-radius:2px;background:{item.color};flex-shrink:0;"></span>
            <span>{item.name}</span>
        </div>
        {/each}
        {#if legendExpanded}
        {#each speciesLegendMore as item}
        <div style="display:flex;align-items:center;gap:6px;white-space:nowrap;opacity:0.85;">
            <span style="width:10px;height:10px;border-radius:2px;background:{item.color};flex-shrink:0;"></span>
            <span>{item.name}</span>
        </div>
        {/each}
        {/if}
        <button
            on:click={() => legendExpanded = !legendExpanded}
            style="margin-top:2px;background:none;border:none;color:rgba(255,255,255,0.75);
                   font-size:12px;padding:0;cursor:pointer;text-align:left;"
        >{legendExpanded ? '접기 ▲' : '· · · 더보기'}</button>
    </div>

    {#if legendHidden}
    <div
        on:touchstart={legendSwipeStart}
        on:mousedown={legendSwipeStart}
        on:click={() => legendHidden = false}
        style="position:fixed;bottom:calc({legendBottom}px + env(safe-area-inset-bottom, 0px));left:0;z-index:150;
                background:rgba(0,0,0,0.65);color:white;
                padding:8px 6px;border-radius:0 10px 10px 0;font-size:12px;
                touch-action:pan-y;cursor:pointer;">▶</div>
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

    .legend-swipe-handle {
        position: absolute;
        right: 6px;
        top: 50%;
        width: 4px;
        height: 36px;
        border-radius: 2px;
        background: #4dabf7;
        transform: translateY(-50%);
        animation: legend-handle-nudge 2.2s ease-in-out infinite;
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
