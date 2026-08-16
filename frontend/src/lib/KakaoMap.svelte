<script context="module" lang="ts">
    declare var kakao: any;
</script>

<script lang="ts">
    import { onMount, createEventDispatcher } from 'svelte';
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
    const OTHER_SPECIES = ['밤나무', '자작나무', '포플러', '오리나무', '벚나무', '물푸레나무'];

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
        '물푸레나무':   { fill: '#78889a', stroke: '#5c6878' }
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
    let watchId: number | null = null;
    let savedLocationMarkers: any[] = [];
    let locationOverlay: any = null;
    let overlayElement: HTMLDivElement | null = null;

    let currentHeading = 0;
    let continuousHeading = 0;
    let rafId: number | null = null;
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

    function applyMapRotation(heading: number) {
        currentHeading = heading;
        const current = ((continuousHeading % 360) + 360) % 360;
        let diff = heading - current;
        if (diff > 180) diff -= 360;
        if (diff < -180) diff += 360;
        continuousHeading += diff;

        if (rafId !== null) cancelAnimationFrame(rafId);
        const angle = continuousHeading;
        rafId = requestAnimationFrame(() => {
            if (overlayElement) {
                overlayElement.style.transition = 'transform 0.2s linear';
                overlayElement.style.transform = `rotate(${-angle}deg)`;
            }
            if (mapContainer) {
                mapContainer.style.transition = 'transform 0.2s linear';
                mapContainer.style.transform = `rotate(${angle}deg) scale(1.5)`;
                mapContainer.style.transformOrigin = '50% 50%';
            }
            rafId = null;
        });
    }

    function createOrUpdateLocationOverlay(lat: number, lng: number) {
        const pos = new kakao.maps.LatLng(lat, lng);
        if (!locationOverlay) {
            const el = document.createElement('div');
            el.style.cssText = 'pointer-events:none;transform-origin:50% 75%;';
            el.innerHTML = `<svg width="56" height="72" viewBox="0 0 56 72" xmlns="http://www.w3.org/2000/svg">
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
        startOrientationTracking();
        locating.set(true);

        navigator.geolocation.getCurrentPosition(
            (position) => {
                const lat = position.coords.latitude;
                const lng = position.coords.longitude;
                currentLat = lat;
                currentLng = lng;
                (map as any).panTo(new kakao.maps.LatLng(lat, lng));
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
                (map as any).panTo(new kakao.maps.LatLng(lat, lng));
                updatePolygonsIfNeeded(lat, lng);

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

    export function stopHeading() {
        isHeadingActive = false;
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
        absoluteOrientationReceived = false;
        if (mapContainer) {
            mapContainer.style.transition = '';
            mapContainer.style.transform = '';
        }
        lastPolygonUpdate = 0;
        lastCenter = null;

        dispatch('headingstop');
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

            kakao.maps.event.addListener(map, 'dragend', fetchAndDrawPolygons);
            kakao.maps.event.addListener(map, 'dragstart', () => {
                stopHeading();
            });
            kakao.maps.event.addListener(map, 'zoom_changed', () => {
                fetchAndDrawPolygons();
            });
            kakao.maps.event.addListener(map, 'click', hidePolygonInfo);
        } else {
            console.error("카카오맵 API 스크립트가 아직 로드되지 않았습니다.");
        }
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
                            showPolygonInfo(item.species, e.latLng);
                        });
                        kakao.maps.event.addListener(polygon, 'mousemove', (e: any) => {
                            if (polygonInfoOverlay) polygonInfoOverlay.setPosition(e.latLng);
                        });
                        kakao.maps.event.addListener(polygon, 'mouseout', hidePolygonInfo);
                        kakao.maps.event.addListener(polygon, 'click', (e: any) => {
                            kakao.maps.event.preventMap();
                            showPolygonInfo(item.species, e.latLng);
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
                });
            }
        };
        document.head.appendChild(script);
        return () => {
            stopHeading();
        };
    });

</script>

<div style="position:relative;width:100%;height:100vh;overflow:hidden;">
    <div bind:this={mapContainer} style="width:100%;height:100%;"></div>

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
