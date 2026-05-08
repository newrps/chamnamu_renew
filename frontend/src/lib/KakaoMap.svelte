<script context="module" lang="ts">
    // 카카오맵 API 객체를 TypeScript에 전역으로 선언하여 타입 에러를 방지합니다.
    declare var kakao: any;


</script>

<script lang="ts">
    import { onMount, createEventDispatcher } from 'svelte';
    import { writable } from 'svelte/store';

    // 스토어: 외부 함수에서 바꿔도 무조건 DOM 반응형 갱신됨
    const headingAngle = writable(0);
    const locating = writable(false); // GPS 정밀 위치 탐색 중 여부
    let instantReset = false; // true일 때 transition 없이 즉시 0° 복귀

    const dispatch = createEventDispatcher();
    const VITE_API_BASE_URL: string = import.meta.env.VITE_API_BASE_URL;
	const VITE_KAKAO_MAP_API_KEY: string = import.meta.env.VITE_KAKAO_MAP_API_KEY;
    let map: kakao.maps.Map;
    let ps: kakao.maps.services.Places;
    let mapContainer: HTMLElement;
    let currentMarker: kakao.maps.Marker | null = null;
    // id → 해당 영역의 폴리곤 목록 (이미 그려진 것 추적)
    let drawnPolygons = new Map<string, kakao.maps.Polygon[]>();
    let polygonsFetchedOnce = false; // 첫 폴리곤 fetch 성공 시 부모에 신호 (prefetch 트리거용)
    
    // 헤딩 기능 관련 변수들
    let isHeadingActive = false;
    let watchId: number | null = null;
    let savedLocationMarkers: any[] = [];

    // 나침반 방향 관련 변수들
    let currentHeading = 0;
    let continuousHeading = 0; // wraparound 없이 누적되는 각도
    let rafId: number | null = null; // rAF throttle용
    let currentLat = 0;
    let currentLng = 0;
    let deviceOrientationAbsoluteHandler: ((e: any) => void) | null = null;
    let deviceOrientationFallbackHandler: ((e: any) => void) | null = null;
    let absoluteOrientationReceived = false;
    let lastHeadingDispatch = 0; // dispatch throttle용
    // rotationWrapper bind:this 대신 Svelte 반응형으로 직접 처리

    let lastPolygonUpdate = 0; // 마지막 폴리곤 업데이트 시간
    let lastCenter: { lat: number, lng: number } | null = null; // 마지막 중심점
    let fetchAbortController: AbortController | null = null; // 진행 중인 fetch 취소용


    function parseGeoJSON(geometry: any): kakao.maps.LatLng[][] {
        if (!geometry) return [];
        const paths: kakao.maps.LatLng[][] = [];
        if (geometry.type === 'MultiPolygon') {
            for (const polygon of geometry.coordinates) {
                const ring: [number, number][] = polygon[0]; // 외곽 링만 사용
                paths.push(ring.map(([lng, lat]) => new kakao.maps.LatLng(lat, lng)));
            }
        } else if (geometry.type === 'Polygon') {
            const ring: [number, number][] = geometry.coordinates[0];
            paths.push(ring.map(([lng, lat]) => new kakao.maps.LatLng(lat, lng)));
        }
        return paths;
    }

    // 지도를 보는 방향으로 회전 — store.set()은 어떤 컨텍스트에서도 DOM을 갱신함
    function applyMapRotation(heading: number) {
        currentHeading = heading;
        // 최단 경로로 누적 각도 업데이트
        const current = ((continuousHeading % 360) + 360) % 360;
        let diff = heading - current;
        if (diff > 180) diff -= 360;
        if (diff < -180) diff += 360;
        continuousHeading += diff;

        // rAF로 화면 주사율에 맞춰 DOM 업데이트 → 지도 이동 중 흔들림 방지
        if (rafId !== null) cancelAnimationFrame(rafId);
        const angle = continuousHeading;
        rafId = requestAnimationFrame(() => {
            headingAngle.set(angle);
            rafId = null;
        });
    }

    // DeviceOrientation API로 나침반 방향 추적 시작
    // iOS 권한은 page.svelte toggleHeading()에서 사용자 gesture 시점에 처리
    function startOrientationTracking() {
        // HTTPS 또는 localhost가 아니면 센서 이벤트가 차단됨
        if (!window.isSecureContext) {
            console.warn('[나침반] 비보안 컨텍스트(HTTP)에서는 방향 센서를 사용할 수 없습니다.');
            alert('나침반 기능은 HTTPS 환경에서만 동작합니다.\n현재 HTTP로 접속 중입니다.');
            return;
        }

        absoluteOrientationReceived = false;

        // Android Chrome: deviceorientationabsolute (절대 방향, 더 정확)
        const handleAbsolute = (event: DeviceOrientationEvent) => {
            if (event.alpha === null) return;
            absoluteOrientationReceived = true;
            const heading = event.alpha;
            currentHeading = heading;
            applyMapRotation(heading);
            // dispatch는 200ms throttle (지도 회전과 분리)
            const now = Date.now();
            if (now - lastHeadingDispatch > 200) {
                lastHeadingDispatch = now;
                dispatch('headingupdate', { lat: currentLat, lng: currentLng, heading });
            }
        };

        // iOS / 일반 fallback
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
    
        // 3초 간격으로만 폴리곤 업데이트
        if (now - lastPolygonUpdate < 3000) {
            return;
        }
    
        // 이전 위치와 현재 위치의 거리 계산 (약 100m 이상 변경시에만)
        if (lastCenter) {
            const distance = getDistance(lastCenter.lat, lastCenter.lng, lat, lng);
            if (distance < 0.001) { // 약 100m
                return;
            }
        }
    
        lastCenter = { lat, lng };
        lastPolygonUpdate = now;
        fetchAndDrawPolygons();
    }

    // 헤딩 기능 시작
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

        // 나침반 방향 추적 시작
        startOrientationTracking();

        locating.set(true);

        // 즉시 대략적인 위치로 이동 (네트워크 기반, 거의 즉시 응답)
        navigator.geolocation.getCurrentPosition(
            (position) => {
                const lat = position.coords.latitude;
                const lng = position.coords.longitude;
                currentLat = lat;
                currentLng = lng;
                const newPos = new kakao.maps.LatLng(lat, lng);
                (map as any).panTo(newPos);
            },
            () => {}, // 실패해도 watchPosition이 이어서 처리
            { enableHighAccuracy: false, maximumAge: 30000, timeout: 3000 }
        );

        let gpsFixed = false; // 첫 GPS 수신 여부
        watchId = navigator.geolocation.watchPosition(
            (position) => {
                if (!gpsFixed) {
                    gpsFixed = true;
                    dispatch('gpsfixed'); // 첫 GPS 수신 알림
                }
                locating.set(false);
                const lat = position.coords.latitude;
                const lng = position.coords.longitude;
                currentLat = lat;
                currentLng = lng;
                const newPos = new kakao.maps.LatLng(lat, lng);

                // 지도 중심을 현재 위치로 이동
                (map as any).panTo(newPos);

                // 폴리곤은 위치가 충분히 변경되었을 때만 업데이트
                updatePolygonsIfNeeded(lat, lng);

                // DeviceOrientation 이벤트가 없으면 GPS 이동방향을 백업으로 사용
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

    // 헤딩 기능 중지
    export function stopHeading() {
        isHeadingActive = false;
        locating.set(false);

        if (watchId !== null) {
            navigator.geolocation.clearWatch(watchId);
            watchId = null;
        }

        // 방향 센서 리스너 제거 (addEventListener와 동일하게 capture: true)
        if (deviceOrientationAbsoluteHandler) {
            window.removeEventListener('deviceorientationabsolute', deviceOrientationAbsoluteHandler as any, true);
            deviceOrientationAbsoluteHandler = null;
        }
        if (deviceOrientationFallbackHandler) {
            window.removeEventListener('deviceorientation', deviceOrientationFallbackHandler, true);
            deviceOrientationFallbackHandler = null;
        }

        // rAF 취소
        if (rafId !== null) {
            cancelAnimationFrame(rafId);
            rafId = null;
        }

        // 지도 회전 초기화
        currentHeading = 0;
        continuousHeading = 0;
        headingAngle.set(0);
        absoluteOrientationReceived = false;

        // 초기화
        lastPolygonUpdate = 0;
        lastCenter = null;

        dispatch('headingstop');
    }

    function initializeMap() {
        if (typeof kakao !== 'undefined' && kakao.maps) {
            const mapOption = {
                center: new kakao.maps.LatLng(37.254971339188, 127.1148388815),
                level: 3
            };
            map = new kakao.maps.Map(mapContainer, mapOption);
            ps = new kakao.maps.services.Places();

            fetchAndDrawPolygons();

            kakao.maps.event.addListener(map, 'dragend', fetchAndDrawPolygons);
            kakao.maps.event.addListener(map, 'dragstart', () => {
                // transition 없이 즉시 0°로 복귀 → 드래그 중 어긋남 방지
                instantReset = true;
                stopHeading();
                requestAnimationFrame(() => { instantReset = false; });
            });
            kakao.maps.event.addListener(map, 'zoom_changed', fetchAndDrawPolygons);
        } else {
            console.error("카카오맵 API 스크립트가 아직 로드되지 않았습니다.");
        }
    }
    function getDistance(lat1: number, lng1: number, lat2: number, lng2: number): number {
        const dLat = lat1 - lat2;
        const dLng = lng1 - lng2;
        return Math.sqrt(dLat * dLat + dLng * dLng);
    }
    // 현재 뷰포트 중심에서 모서리까지의 거리를 미터 단위로 계산 (Haversine)
    function getViewportRadius(): number {
        const center = map.getCenter();
        const bounds = map.getBounds() as any;
        const ne = bounds.getNorthEast();

        const R = 6371000;
        const lat1 = center.getLat() * Math.PI / 180;
        const lat2 = ne.getLat() * Math.PI / 180;
        const dLat = lat2 - lat1;
        const dLng = (ne.getLng() - center.getLng()) * Math.PI / 180;

        const a = Math.sin(dLat / 2) ** 2 +
                  Math.cos(lat1) * Math.cos(lat2) * Math.sin(dLng / 2) ** 2;
        const c = 2 * Math.atan2(Math.sqrt(a), Math.sqrt(1 - a));
        return Math.min(R * c, 50000); // 최대 50km
    }

    // API를 호출하여 폴리곤 데이터를 가져오고 지도에 그리는 함수
    async function fetchAndDrawPolygons() {
        if (!map) return;

        // 이전 요청이 진행 중이면 취소 (경쟁 조건 방지)
        if (fetchAbortController) fetchAbortController.abort();
        fetchAbortController = new AbortController();
        const thisController = fetchAbortController;
        const signal = thisController.signal;

        const center = map.getCenter();
        const lng = center.getLng();
        const lat = center.getLat();
        const distance = getViewportRadius();
        const apiUrl = `${VITE_API_BASE_URL}/api/polygon/nearby?lng=${lng}&lat=${lat}&distance=${distance}`;

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

            // 취소된 요청이면 무시
            if (signal.aborted) return;

            // 이번 응답에 있는 id 목록
            const incomingIds = new Set<string>();

            data.forEach((item: any) => {
                const key = String(item.id);
                incomingIds.add(key);

                // 이미 그려진 폴리곤은 건너뜀
                if (drawnPolygons.has(key)) return;

                // 새로운 영역만 그리기
                const paths = parseGeoJSON(item.geometry);
                const newPolys: kakao.maps.Polygon[] = [];
                paths.forEach((path: kakao.maps.LatLng[]) => {
                    if (path.length > 0) {
                        const polygon = new kakao.maps.Polygon({
                            path,
                            strokeWeight: 3,
                            strokeColor: '#1565c0',
                            strokeOpacity: 0.9,
                            fillColor: '#2196f3',
                            fillOpacity: 0.35
                        });
                        polygon.setMap(map);
                        newPolys.push(polygon);
                    }
                });
                drawnPolygons.set(key, newPolys);
            });

            // 화면 밖으로 나간 영역만 제거
            for (const [key, polys] of drawnPolygons) {
                if (!incomingIds.has(key)) {
                    polys.forEach(p => p.setMap(null));
                    drawnPolygons.delete(key);
                }
            }

            // 첫 fetch 성공 시점 — 부모가 forecast prefetch 트리거에 사용
            if (!polygonsFetchedOnce) {
                polygonsFetchedOnce = true;
                dispatch('polygonsfetched');
            }
        } catch (error: any) {
            clearTimeout(timeoutId);
            if (error?.name === 'AbortError' && !timedOut) {
                // 새 요청으로 인한 취소 — 정상
                return;
            }
            console.error('폴리곤 데이터를 불러오는 데 실패했습니다:', error);
            dispatch('fetcherror');
        }
    }

    export function retryFetchPolygons() {
        fetchAndDrawPolygons();
    }

    // 좌표 → 주소 변환 (역지오코딩)
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

    // 부모 컴포넌트에서 호출할 수 있는 검색 함수입니다.
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

    // 저장된 위치 마커 표시
    export function setSavedLocations(locations: { id: number; name: string; lat: number; lng: number }[]) {
        // 기존 마커 제거
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

    // 부모 컴포넌트에서 호출할 수 있는 지도 중심 이동 함수입니다.
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
        // 카카오맵 스크립트를 동적으로 로드합니다.
        const script = document.createElement('script');
        script.src = `//dapi.kakao.com/v2/maps/sdk.js?appkey=${VITE_KAKAO_MAP_API_KEY}&autoload=false&libraries=services`;
        script.async = true;

        // 스크립트 로드가 완료되면 지도를 초기화합니다.
        script.onload = () => {
            if (typeof kakao !== 'undefined' && kakao.maps) {
                kakao.maps.load(() => {
                    initializeMap();
                });
            }
        };

        // 스크립트를 문서의 head에 추가합니다.
        document.head.appendChild(script);
        
        // 컴포넌트 언마운트 시 헤딩 기능 정리
        return () => {
            stopHeading();
        };
    });

</script>

<!-- 바깥 wrapper: 화면 크기 고정 + 넘치는 부분 자르기 -->
<div style="position:relative;width:100%;height:100vh;overflow:hidden;">

    <!-- 지도 회전 div: 대각선보다 크게 만들어 모서리 검정 방지 -->
    <div
        style="position:absolute;top:50%;left:50%;width:150vmax;height:150vmax;margin-top:-75vmax;margin-left:-75vmax;transform-origin:center center;will-change:transform;"
        style:transition={instantReset ? 'none' : 'transform 0.05s ease-out'}
        style:transform="rotate({$headingAngle}deg)"
    >
        <div bind:this={mapContainer} style="width:100%;height:100%;"></div>
    </div>

    <!-- GPS 정밀 위치 탐색 중 로딩 표시 -->
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

    <!-- 현위치 + 방향 아이콘: 회전 div 밖 → 항상 보는 방향이 위 -->
    {#if isHeadingActive}
    <div style="position:absolute;top:50%;left:50%;transform:translate(-50%,-60%);z-index:100;pointer-events:none;will-change:transform;">
        <svg width="56" height="72" viewBox="0 0 56 72" xmlns="http://www.w3.org/2000/svg">
            <!-- 방향 빔: \ / 모양, 보는 방향(위쪽)으로 퍼짐 -->
            <polygon points="28,46 2,2 54,2"
                fill="rgba(66,133,244,0.35)"
                stroke="rgba(66,133,244,0.5)"
                stroke-width="1"
                stroke-linejoin="round"/>
            <!-- 현위치 외곽 흰 테두리 -->
            <circle cx="28" cy="54" r="15" fill="white"
                style="filter:drop-shadow(0 2px 6px rgba(0,0,0,0.35))"/>
            <!-- 현위치 파란 점 -->
            <circle cx="28" cy="54" r="11" fill="#4285f4"/>
            <!-- 중심 흰 점 -->
            <circle cx="28" cy="54" r="4" fill="white"/>
        </svg>
    </div>
    {/if}

</div>

<style>
    @keyframes spin {
        from { transform: rotate(0deg); }
        to   { transform: rotate(360deg); }
    }
</style>