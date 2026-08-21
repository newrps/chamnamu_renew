<script lang="ts">
    import KakaoMap from '$lib/KakaoMap.svelte';
    import CollectingForecast from '$lib/CollectingForecast.svelte';
    import { onMount } from 'svelte';
    import { authUser, initAuth, logout, saveLocation, fetchLocations, deleteLocation } from '$lib/auth';
    import { prefetchForecast } from '$lib/forecastCache';
    import type { SavedLocation } from '$lib/auth';

    let mapComponent: KakaoMap;
    let searchQuery = '';
    let searchResults: any[] = [];
    let hasSearched = false;
    let showSearchResults = false;

    let isHeadingActive = false;
    let headingMode: 'off' | 'north-up' | 'heading-up' = 'off';
    let isFollowing = true; // false면 드래그로 화면을 옮겨서 재중심이 일시정지된 상태
    let isGPSLocating = false; // GPS 정밀 위치 탐색 중
    let isSatellite = false;
    let roadviewMode = false;
    let currentLocation = { lat: 0, lng: 0, heading: 0 };

    $: headingButtonTitle = !isHeadingActive
        ? '현재 위치 및 방향 표시'
        : !isFollowing
            ? '현재 위치로 돌아가기'
            : headingMode === 'north-up'
                ? '바라보는 방향으로 지도 회전'
                : '현재 위치 모드 끄기';

    let showCollectingPanel = false;
    let forecastLat = 0;
    let forecastLng = 0;

    // GPS 좌표가 생기면 예보 위치 초기값으로 사용
    $: if (forecastLat === 0 && currentLocation.lat !== 0) {
        forecastLat = currentLocation.lat;
        forecastLng = currentLocation.lng;
    }

    // 지도·폴리곤 로드 완료 후 백그라운드 forecast prefetch.
    // KakaoMap 의 polygonsfetched 이벤트로 신호 받음. (5초 내 못 받으면 안전망 강제 발동)
    let polygonsReady = false;
    onMount(() => {
        setTimeout(() => { polygonsReady = true; }, 5000);
    });

    $: if (polygonsReady && forecastLat !== 0 && forecastLng !== 0) {
        prefetchForecast(forecastLat, forecastLng);
    }
    let polygonFetchFailed = false;
    let polygonRetryTimer: ReturnType<typeof setTimeout> | null = null;

    function onPolygonFetchError() {
        polygonFetchFailed = true;
        if (polygonRetryTimer) clearTimeout(polygonRetryTimer);
        polygonRetryTimer = setTimeout(() => { polygonFetchFailed = false; }, 8000);
    }

    function retryPolygons() {
        polygonFetchFailed = false;
        if (polygonRetryTimer) clearTimeout(polygonRetryTimer);
        if (mapComponent) mapComponent.retryFetchPolygons();
    }

    // 로그인/위치 저장 관련
    let showLoginModal = false;
    let showLocationsPanel = false;
    let showUserMenu = false;
    let savedLocations: SavedLocation[] = [];
    let saveLocationName = '';
    let showSaveForm = false;
    let expandedLocationId: number | null = null;
    let locationAddresses: Record<number, string> = {};

    async function loadSavedLocations() {
        savedLocations = await fetchLocations();
        if (mapComponent) mapComponent.setSavedLocations(savedLocations);
    }

    async function handleSaveLocation() {
        if (!saveLocationName.trim()) return;
        const loc = await saveLocation(
            saveLocationName.trim(),
            currentLocation.lat,
            currentLocation.lng
        );
        if (loc) {
            savedLocations = [loc, ...savedLocations];
            if (mapComponent) mapComponent.setSavedLocations(savedLocations);
            saveLocationName = '';
            showSaveForm = false;
        }
    }

    async function handleDeleteLocation(id: number) {
        if (!confirm('저장된 위치를 삭제하시겠습니까?')) return;
        const ok = await deleteLocation(id);
        if (ok) {
            savedLocations = savedLocations.filter(l => l.id !== id);
            if (mapComponent) mapComponent.setSavedLocations(savedLocations);
        }
    }

    function handleLocationMarkerClick(loc: SavedLocation) {
        if (isHeadingActive && mapComponent) {
            mapComponent.stopHeading();
            isGPSLocating = false;
        }
        if (mapComponent) mapComponent.setCenter(loc.lat, loc.lng);
    }

    async function toggleLocationExpand(loc: SavedLocation) {
        if (expandedLocationId === loc.id) {
            expandedLocationId = null;
            return;
        }
        expandedLocationId = loc.id;
        // 주소 캐시 없으면 조회
        if (!locationAddresses[loc.id] && mapComponent) {
            locationAddresses[loc.id] = '주소 조회 중...';
            const addr = await mapComponent.getAddressFromCoords(loc.lat, loc.lng);
            locationAddresses = { ...locationAddresses, [loc.id]: addr };
        }
    }

    async function copyAddress(loc: SavedLocation) {
        const addr = locationAddresses[loc.id];
        if (!addr) return;
        await navigator.clipboard.writeText(addr);
        alert('주소가 복사되었습니다.');
    }

    function openKakaoNavi(loc: SavedLocation) {
        // 앱 설치 시 앱으로, 미설치 시 웹으로 자동 처리
        window.open(`https://map.kakao.com/link/to/${encodeURIComponent(loc.name)},${loc.lat},${loc.lng}`, '_blank');
    }

    function openNaverNavi(loc: SavedLocation) {
        // 앱 설치 시 앱으로, 미설치 시 웹으로 자동 처리
        window.open(`https://map.naver.com/v5/directions/-/${loc.lng},${loc.lat},${encodeURIComponent(loc.name)}/-/transit`, '_blank');
    }

    function openTmap(loc: SavedLocation) {
        // iOS와 Android 파라미터 이름이 다름
        const name = encodeURIComponent(loc.name);
        const isIOS = /iPhone|iPad|iPod/i.test(navigator.userAgent);
        if (isIOS) {
            window.open(`tmap://route?rGoName=${name}&rGoX=${loc.lng}&rGoY=${loc.lat}`, '_blank');
        } else {
            window.open(`tmap://route?goalname=${name}&goalx=${loc.lng}&goaly=${loc.lat}`, '_blank');
        }
    }

    // 쿠키 유틸리티
    function setCookie(name: string, value: string, days: number) {
        const expires = new Date();
        expires.setDate(expires.getDate() + days);
        document.cookie = `${name}=${value};expires=${expires.toUTCString()};path=/`;
    }
    function getCookie(name: string): string | null {
        const match = document.cookie.match(new RegExp('(^| )' + name + '=([^;]+)'));
        return match ? match[2] : null;
    }

    // 모바일 스와이프 관련 변수들
    let searchContainer: HTMLElement;
    let isHidden = false;
    let startY = 0;
    let currentY = 0;
    let isDragging = false;

    // 모바일 좌우 컨트롤 묶음 스와이프
    let leftControlsHidden = false;
    let rightControlsHidden = false;
    let overviewMapHidden = false;
    let menuStateReady = false;
    let controlSwipeSide: 'left' | 'right' | null = null;
    let controlSwipeStartX = 0;
    let controlSwipeStartY = 0;
    let controlSwipeTriggered = false;

    function setControlsHidden(side: 'left' | 'right', hidden: boolean) {
        if (side === 'left') {
            leftControlsHidden = hidden;
            setCookie('map_left_controls_hidden', hidden ? '1' : '0', 30);
            if (hidden) showCollectingPanel = false;
        } else {
            rightControlsHidden = hidden;
            setCookie('map_right_controls_hidden', hidden ? '1' : '0', 30);
            if (hidden) showSaveForm = false;
        }
    }

    function onLegendVisibilityChange(event: CustomEvent<{ hidden: boolean }>) {
        leftControlsHidden = event.detail.hidden;
        setCookie('map_left_controls_hidden', event.detail.hidden ? '1' : '0', 30);
    }

    function onOverviewVisibilityChange(event: CustomEvent<{ hidden: boolean }>) {
        overviewMapHidden = event.detail.hidden;
        setCookie('map_overview_hidden', event.detail.hidden ? '1' : '0', 30);
    }

    function handleControlSwipeStart(event: TouchEvent, side: 'left' | 'right') {
        if (event.touches.length !== 1) return;
        if ((event.target as HTMLElement).closest('input')) return;
        controlSwipeSide = side;
        controlSwipeStartX = event.touches[0].clientX;
        controlSwipeStartY = event.touches[0].clientY;
        controlSwipeTriggered = false;
        document.addEventListener('touchmove', handleControlSwipeMove, { passive: false });
        document.addEventListener('touchend', handleControlSwipeEnd, { once: true });
        document.addEventListener('touchcancel', handleControlSwipeEnd, { once: true });
    }

    function handleControlSwipeMove(event: TouchEvent) {
        if (!controlSwipeSide || event.touches.length !== 1) return;
        if (controlSwipeTriggered) return;
        const deltaX = event.touches[0].clientX - controlSwipeStartX;
        const deltaY = event.touches[0].clientY - controlSwipeStartY;
        if (Math.abs(deltaX) < 45 || Math.abs(deltaX) <= Math.abs(deltaY) * 1.2) return;

        const shouldHide = controlSwipeSide === 'left' ? deltaX < 0 : deltaX > 0;
        const shouldShow = controlSwipeSide === 'left' ? deltaX > 0 : deltaX < 0;
        const isHidden = controlSwipeSide === 'left' ? leftControlsHidden : rightControlsHidden;
        if ((!isHidden && shouldHide) || (isHidden && shouldShow)) {
            event.preventDefault();
            controlSwipeTriggered = true;
            setControlsHidden(controlSwipeSide, !isHidden);
        }
    }

    function handleControlSwipeEnd(event?: Event) {
        if (controlSwipeTriggered) event?.preventDefault();
        controlSwipeSide = null;
        controlSwipeTriggered = false;
        document.removeEventListener('touchmove', handleControlSwipeMove);
        document.removeEventListener('touchend', handleControlSwipeEnd);
        document.removeEventListener('touchcancel', handleControlSwipeEnd);
    }


    // 광고 배너 타입 정의
    interface AdBanner {
        type: 'text' | 'image';
        url: string;
        content: string;
        backgroundColor: string;
    }

    // 광고 배너 관련 변수들
    let showAdBanner = false;
    let currentAd: AdBanner | null = null; // 현재 선택된 광고
    let adList: AdBanner[] = []; // JSON에서 불러올 광고 목록

    // 우측 하단 FAB 스택 위치 계산 (채집예보/나침반 -> 위성 -> 로드뷰 -> 저장위치 순으로 쌓임)
    $: fabBaseBottom = showAdBanner ? 116 : 20;
    $: saveLocationFabBottom = fabBaseBottom + 176;
    $: locationsFabBottom = isHeadingActive ? fabBaseBottom + 232 : fabBaseBottom + 176;
    $: rightControlsTopOffset = $authUser ? (isHeadingActive ? 232 : 176) : 112;
    $: rightControlsGroupHeight = rightControlsTopOffset + 60;
    $: rightControlsGripBottom = fabBaseBottom - 6 + Math.max(0, (rightControlsGroupHeight - 112) / 2);

    // 광고 목록을 불러오는 함수 (API 우선, 실패 시 ads.json 폴백)
    async function loadAds() {
        try {
            const response = await fetch('/api/ads');
            if (!response.ok) throw new Error('API 실패');
            const data: AdBanner[] = await response.json();
            if (data.length > 0) {
                adList = data;
                return;
            }
            throw new Error('광고 없음');
        } catch {
            // 폴백: 정적 ads.json 사용
            try {
                const response = await fetch('/ads.json');
                if (!response.ok) throw new Error('ads.json 실패');
                const data = await response.json();
                adList = data.ads;
            } catch (e) {
                console.error('광고 로드 실패:', e);
            }
        }
    }

    function handleSearch() {
        hasSearched = true;
        if (searchQuery.trim() !== '') {
            // 헤딩 기능이 켜져 있으면 끄기
            if (isHeadingActive && mapComponent) {
                mapComponent.stopHeading();
            }

            if (mapComponent) {
                mapComponent.search(searchQuery);
        
            }
        }
    }

    function onSearchResults(event: CustomEvent<{ results: any[] }>) {
        searchResults = event.detail.results;
        showSearchResults = true;

    }

    function handleResultClick(result: any) {
        // 헤딩 기능이 켜져 있으면 끄기
        if (isHeadingActive && mapComponent) {
            mapComponent.stopHeading();
        }

        if (mapComponent) {
            mapComponent.setCenter(result.y, result.x);
        }
        forecastLat = Number(result.y);
        forecastLng = Number(result.x);
        showSearchResults = false;

    }

    function handleClickOutside(event: MouseEvent) {
        if (searchContainer && !searchContainer.contains(event.target as Node)) {
            showSearchResults = false;
        }
        const userMenuEl = document.querySelector('.user-menu-wrap');
        if (userMenuEl && !userMenuEl.contains(event.target as Node)) {
            showUserMenu = false;
        }
        const saveFormEl = document.querySelector('.save-form-popup');
        if (saveFormEl && !saveFormEl.contains(event.target as Node)) {
            const fabEl = document.querySelector('.save-location-fab');
            if (!fabEl || !fabEl.contains(event.target as Node)) {
                showSaveForm = false;
            }
        }
    }

    function handleKeyDown(event: KeyboardEvent) {
        if (event.key === 'Enter') {
            handleSearch();
        }
    }

    // 현재 위치 버튼: 꺼짐 -> 북쪽 고정(방향 표시) -> 헤딩업(지도 회전) -> 꺼짐
    async function toggleHeading() {
        // iOS 13+: DeviceOrientation 권한은 사용자 gesture에서 직접 호출해야 함
        if (headingMode === 'off' && typeof (DeviceOrientationEvent as any).requestPermission === 'function') {
            try {
                const perm = await (DeviceOrientationEvent as any).requestPermission();
                if (perm !== 'granted') {
                    alert('방향 센서 권한이 필요합니다. 설정 > Safari > 동작 및 방향 접근에서 허용해 주세요.');
                    return;
                }
            } catch (e) {
                console.error('방향 센서 권한 요청 실패:', e);
            }
        }
        if (mapComponent) {
            if (isHeadingActive && !isFollowing) {
                // 드래그로 중심 추적이 멈췄으면 모드를 바꾸기 전에 현재 위치로 먼저 복귀한다.
                mapComponent.resumeFollowing();
            } else if (headingMode === 'off') {
                mapComponent.startHeading('north-up');
                isGPSLocating = true; // GPS 잡을 때까지 스피닝
            } else if (headingMode === 'north-up') {
                mapComponent.setHeadingMode('heading-up');
            } else {
                mapComponent.stopHeading();
                isGPSLocating = false;
            }
        }

    }

    // 헤딩 업데이트 이벤트 처리
    function onHeadingUpdate(event: CustomEvent<{ lat: number, lng: number, heading: number, accuracy?: number }>) {
        currentLocation = event.detail;
        isHeadingActive = true;
        // 나침반 켜면 예보 위치도 현재 GPS 위치로 업데이트
        forecastLat = event.detail.lat;
        forecastLng = event.detail.lng;
    }

    function onHeadingModeChange(event: CustomEvent<{ mode: 'north-up' | 'heading-up' }>) {
        headingMode = event.detail.mode;
        isHeadingActive = true;
    }

    // 헤딩 중지 이벤트 처리
    function onHeadingStop() {
        headingMode = 'off';
        isHeadingActive = false;
        isFollowing = true;
        isGPSLocating = false;
    }

    // 드래그로 재중심이 일시정지/재개될 때
    function onFollowChange(event: CustomEvent<{ following: boolean }>) {
        isFollowing = event.detail.following;
    }

    // 드래그 핸들 전용 이벤트 핸들러
    function handleDragStart(event: TouchEvent | MouseEvent) {
        event.preventDefault();
        event.stopPropagation();
        
        if (event instanceof TouchEvent) {
            startY = event.touches[0].clientY;
            document.addEventListener('touchmove', handleDragMove, { passive: false });
            document.addEventListener('touchend', handleDragEnd, { once: true });
        } else {
            startY = (event as MouseEvent).clientY;
            document.addEventListener('mousemove', handleDragMove);
            document.addEventListener('mouseup', handleDragEnd, { once: true });
        }
        isDragging = true;
    }

    function handleDragMove(event: TouchEvent | MouseEvent) {
        if (!isDragging) return;
        
        event.preventDefault();
        event.stopPropagation();
        
        if (event instanceof TouchEvent) {
            currentY = event.touches[0].clientY;
        } else {
            currentY = (event as MouseEvent).clientY;
        }
        
        const deltaY = currentY - startY;

        // 드래그 임계값
        if (deltaY < -50 && !isHidden) {
            isHidden = true;
        } else if (deltaY > 50 && isHidden) {
            isHidden = false;
        }
    }

    function handleDragEnd() {
        isDragging = false;
        startY = 0;
        currentY = 0;

        // 패널 상태 쿠키 저장 (30일)
        setCookie('search_panel_hidden', isHidden ? '1' : '0', 30);

        // 임시 이벤트 리스너 제거
        document.removeEventListener('touchmove', handleDragMove);
        document.removeEventListener('mousemove', handleDragMove);
    }

    // 전체 스크롤 방지
    function preventBodyScroll() {
        document.body.style.overflow = 'hidden';
        document.body.style.position = 'fixed';
        document.body.style.width = '100%';
    }

    function restoreBodyScroll() {
        document.body.style.overflow = '';
        document.body.style.position = '';
        document.body.style.width = '';
    }

    // 광고 배너 관련 함수들
    function getRandomAd(): AdBanner | null {
        if (adList.length === 0) return null;
        const randomIndex = Math.floor(Math.random() * adList.length);
        return adList[randomIndex];
    }

    function checkAdBannerStatus(): void {
        const hiddenUntil = sessionStorage.getItem('adBannerHiddenUntil');
        if (hiddenUntil && new Date().getTime() < parseInt(hiddenUntil)) {
            showAdBanner = false;
        } else {
            showAdBanner = true;
            currentAd = getRandomAd(); // 랜덤 광고 선택
        }
    }

    function handleAdBannerClick(): void {
        if (!currentAd) return;
        
        // 새창으로 링크 열기
        window.open(currentAd.url, '_blank');
        
        // 광고 배너 숨기기
        showAdBanner = false;
        
        // 1시간(3600000ms) 후까지 광고 숨김
        const hideUntil = new Date().getTime() + 3600000;
        sessionStorage.setItem('adBannerHiddenUntil', hideUntil.toString());
    }

    onMount(() => {
        preventBodyScroll();

        // 검색 패널 상태 복원
        if (getCookie('search_panel_hidden') === '1') isHidden = true;
        leftControlsHidden = getCookie('map_left_controls_hidden') === '1';
        rightControlsHidden = getCookie('map_right_controls_hidden') === '1';
        overviewMapHidden = getCookie('map_overview_hidden') === '1';
        requestAnimationFrame(() => { menuStateReady = true; });

        // 인증 초기화 (URL 토큰 처리 + localStorage 복원)
        initAuth();

        // 로그인 상태면 저장 위치 로드
        const unsubscribe = authUser.subscribe((user: typeof $authUser) => {
            if (user) loadSavedLocations();
        });

        // 광고 로드
        const initializeAds = async () => {
            await loadAds();
            checkAdBannerStatus();
        };
        initializeAds();

        // 5분마다 광고 갱신
        const adInterval = setInterval(async () => {
            await loadAds();
            currentAd = getRandomAd();
        }, 5 * 60 * 1000);

        return () => {
            unsubscribe();
            clearInterval(adInterval);
            restoreBodyScroll();
        };
    });
</script>

<svelte:body on:click={handleClickOutside}/>

<style>
    .page-container {
        display: flex;
        flex-direction: column;
        height: 100vh; /* min-height에서 height로 변경 */
        overflow: hidden; /* 전체 스크롤 차단 */
    }

    .header {
        background: #f8f9fa;
        padding: 15px 20px;
        text-align: center;
        border-bottom: 1px solid #ddd;
        position: relative;
        z-index: 300;
    }

    .header h1 {
        font-size: 20px;
        margin: 0;
        color: #2c3e50;
    }

    .engine-switch-badge {
        display: block;
        font-size: 11px;
        font-weight: 400;
        margin-top: 4px;
    }
    .engine-switch-badge a {
        color: #4dabf7;
        text-decoration: none;
    }
    .engine-switch-badge a:hover {
        text-decoration: underline;
    }

    .header .subtitle {
        font-size: 14px;
        margin-top: 8px;
        color: #555;
        line-height: 1.6;
    }

    .map-wrapper {
        flex-grow: 1;
        position: relative;
        height: 100%; /* min-height 제거하고 100%로 설정 */
        overflow: hidden; /* 지도 영역 스크롤 차단 */
    }

    .search-container {
        position: absolute;
        top: 20px; /* 20px에서 60px로 더 아래로 이동 */
        left: 50%;
        transform: translateX(-50%);
        z-index: 10;
        display: flex;
        flex-direction: column;
        width: 350px;
        max-height: 80vh;
        background-color: white;
        border-radius: 8px;
        box-shadow: 0 4px 10px rgba(0, 0, 0, 0.2);
        padding: 10px;
        box-sizing: border-box;
        transition: transform 0.3s ease;
        user-select: none;
    }

    .search-container.hidden {
        transform: translateX(-50%) translateY(-120%);
    }

    /* 드래그 핸들 영역 */
    .drag-handle {
        position: absolute;
        top: -20px;
        left: 50%;
        transform: translateX(-50%);
        width: 120px; /* 60px에서 120px로 확장 */
        height: 25px; /* 20px에서 25px로 확장 */
        cursor: grab;
        display: flex;
        align-items: center;
        justify-content: center;
        border-radius: 12px 12px 0 0;
        background: transparent;
        z-index: 2;
        touch-action: none;
    }

    .drag-handle:active {
        cursor: grabbing;
    }

    .drag-handle::before {
        content: '';
        width: 60px; /* 30px에서 60px로 확장 */
        height: 4px; /* 3px에서 4px로 확장 */
        background-color: #007bff;
        border-radius: 2px;
        opacity: 0.8;
    }

    .search-container.hidden .drag-handle {
        top: calc(100%);
        border-radius: 0 0 12px 12px;
        background: transparent;
        animation: bounce 2s infinite;
    }

    .search-container.hidden .drag-handle::before {
        background-color: #007bff;
    }

    @keyframes bounce {
        0%, 20%, 50%, 80%, 100% { 
            transform: translateX(-50%) translateY(0); 
        }
        40% { 
            transform: translateX(-50%) translateY(-8px); 
        }
        60% { 
            transform: translateX(-50%) translateY(-4px); 
        }
    }



    @media (max-width: 768px) {
        .search-container {
            width: calc(100% - 20px);
            /*top: 50px;*/ /* 모바일에서도 더 아래로 */
        }

        .drag-handle {
            display: flex;
        }
    }

    @media (min-width: 769px) {
        .drag-handle {
            display: none;
        }
    }

    .search-input-box {
        display: flex;
        gap: 5px;
        margin-bottom: 10px;
        margin-top: 10px;
    }

    .search-input-box input {
        flex-grow: 1;
        padding: 8px;
        border: 1px solid #ccc;
        border-radius: 4px;
    }

    .search-input-box button {
        padding: 8px 12px;
        border: none;
        background-color: #007bff;
        color: white;
        border-radius: 4px;
        cursor: pointer;
    }

    .button-row {
        display: flex;
        gap: 5px;
        margin-bottom: 10px;
    }

    .location-button {
        flex: 1;
        padding: 8px 12px;
        border: none;
        background-color: #28a745;
        color: white;
        border-radius: 4px;
        cursor: pointer;
        font-size: 12px;
    }

    /* 나침반 floating 버튼 */
    .collecting-fab {
        position: fixed;
        left: 16px;
        width: 48px;
        height: 48px;
        border-radius: 50%;
        border: none;
        background: white;
        box-shadow: 0 2px 10px rgba(0,0,0,0.2);
        font-size: 22px;
        cursor: pointer;
        z-index: 100;
        display: flex;
        align-items: center;
        justify-content: center;
        color: #35644a;
        transition: background 0.2s, transform 0.28s ease, opacity 0.2s ease;
    }
    .collecting-fab.left-controls-hidden {
        transform: translateX(-80px);
        opacity: 0;
        pointer-events: none;
    }
    .collecting-fab.active {
        background: #1b5e20;
        color: white;
    }

    .compass-fab {
        position: fixed;
        right: 12px;
        bottom: 80px;
        width: 48px;
        height: 48px;
        border-radius: 50%;
        border: none;
        background: white;
        box-shadow: 0 2px 8px rgba(0,0,0,0.3);
        cursor: pointer;
        display: flex;
        align-items: center;
        justify-content: center;
        z-index: 5;
        color: #555;
        transition: all 0.3s ease;
        padding: 0;
    }

    .compass-fab:hover {
        box-shadow: 0 4px 12px rgba(0,0,0,0.35);
    }

    .compass-fab.active {
        background: #1a73e8;
        color: white;
        box-shadow: 0 2px 12px rgba(26,115,232,0.5);
    }

    .compass-fab.active.heading-up {
        background: #0d47a1;
        box-shadow: 0 2px 12px rgba(13,71,161,0.55);
    }

    /* 드래그로 재중심이 일시정지된 상태 - 다시 누르면 현재 위치로 돌아감 */
    .compass-fab.active.paused {
        background: white;
        color: #1a73e8;
        border: 2px solid #1a73e8;
    }

    .compass-fab.locating {
        animation: compass-spin 1s linear infinite;
    }

    @keyframes compass-spin {
        from { transform: rotate(0deg); }
        to   { transform: rotate(360deg); }
    }

    .satellite-fab {
        position: fixed;
        right: 12px;
        width: 48px;
        height: 48px;
        border-radius: 50%;
        border: none;
        background: white;
        box-shadow: 0 2px 8px rgba(0,0,0,0.3);
        cursor: pointer;
        display: flex;
        align-items: center;
        justify-content: center;
        z-index: 5;
        color: #4b5563;
        font-size: 22px;
        transition: all 0.3s ease;
        padding: 0;
    }

    .satellite-fab:hover {
        box-shadow: 0 4px 12px rgba(0,0,0,0.35);
    }

    .satellite-fab.active {
        background: #1a73e8;
        color: white;
        box-shadow: 0 2px 12px rgba(26,115,232,0.5);
    }

    .control-fab-icon {
        width: 25px;
        height: 25px;
        display: block;
        pointer-events: none;
    }

    .right-controls-layer {
        position: fixed;
        inset: 0;
        z-index: 160;
        pointer-events: none;
        transition: transform 0.28s ease;
        will-change: transform;
    }
    .menu-state-restoring {
        visibility: hidden;
        transition: none !important;
    }
    .right-controls-layer > * {
        pointer-events: auto;
    }
    .right-controls-layer.hidden {
        transform: translateX(calc(100% - 22px));
    }
    .right-controls-edge-tab {
        position: fixed;
        right: 0;
        top: 52%;
        transform: translateY(-50%);
        width: 22px;
        height: 64px;
        padding: 0;
        border: 0;
        border-radius: 10px 0 0 10px;
        background: rgba(0, 0, 0, 0.62);
        color: white;
        font-size: 13px;
        box-shadow: -2px 0 8px rgba(0, 0, 0, 0.2);
        touch-action: pan-y;
        z-index: 161;
    }
    .right-controls-group-bg {
        position: absolute;
        right: 6px;
        width: 60px;
        border-radius: 32px;
        background: rgba(255, 255, 255, 0.76);
        border: 1px solid rgba(255, 255, 255, 0.9);
        box-shadow: 0 3px 16px rgba(0, 0, 0, 0.2);
        backdrop-filter: blur(8px);
        -webkit-backdrop-filter: blur(8px);
        z-index: 0;
        touch-action: pan-y;
    }
    .right-controls-swipe-hint {
        position: absolute;
        right: 64px;
        width: 20px;
        height: 112px;
        padding: 0;
        border: 0;
        background: transparent;
        display: flex;
        align-items: center;
        justify-content: center;
        cursor: pointer;
        touch-action: pan-y;
        animation: right-swipe-hint 2s ease-in-out infinite;
        z-index: 6;
    }
    .right-controls-swipe-hint::before {
        content: '';
        width: 4px;
        height: 112px;
        border-radius: 2px;
        background: #4dabf7;
        box-shadow: 0 0 6px rgba(77, 171, 247, 0.65);
    }
    @keyframes right-swipe-hint {
        0%, 70%, 100% { transform: translateX(0); opacity: 0.65; }
        82% { transform: translateX(3px); opacity: 1; }
    }

    .search-results-list {
        list-style: none;
        padding: 0;
        margin: 0;
        overflow-y: auto;
        touch-action: pan-y; /* 세로 스크롤만 허용 */
    }

    .search-results-list li {
        padding: 10px;
        cursor: pointer;
        border-bottom: 1px solid #eee;
    }

    .search-results-list li:hover {
        background-color: #f5f5f5;
    }

    .search-results-list .place-name {
        font-weight: bold;
    }

    .description {
        padding: 20px;
        font-size: 15px;
        line-height: 1.6;
        color: #333;
        background: #fafafa;
        border-top: 1px solid #ddd;
    }

    /* 광고 배너 스타일 */
    .ad-banner {
        position: fixed;
        bottom: 0;
        left: 0;
        right: 0;
        color: white;
        padding: 12px 20px;
        text-align: center;
        cursor: pointer;
        z-index: 1000;
        font-weight: bold;
        font-size: 14px;
        box-shadow: 0 -2px 10px rgba(0, 0, 0, 0.2);
        animation: slideInUp 0.5s ease-out, pulse-glow 3s infinite;
        transition: transform 0.3s ease;
        display: block;
        box-sizing: border-box;
    }

    .ad-banner:hover {
        transform: translateY(-2px);
    }

    .ad-banner:active {
        transform: translateY(0);
    }

    .ad-banner img {
        max-width: 100%;
        max-height: 40px;
        object-fit: contain;
    }

    @keyframes slideInUp {
        from {
            transform: translateY(100%);
            opacity: 0;
        }
        to {
            transform: translateY(0);
            opacity: 1;
        }
    }

    @keyframes pulse-glow {
        0%, 100% {
            box-shadow: 0 -2px 10px rgba(0, 0, 0, 0.2);
        }
        50% {
            box-shadow: 0 -2px 20px rgba(255, 107, 107, 0.4), 0 -2px 10px rgba(0, 0, 0, 0.2);
        }
    }

    /* 광고 배너가 있을 때 지도 영역 조정 */
    .map-wrapper.with-ad {
        padding-bottom: 60px;
    }

    .polygon-error-toast {
        position: fixed;
        bottom: 100px;
        left: 50%;
        transform: translateX(-50%);
        display: flex;
        align-items: center;
        gap: 10px;
        padding: 10px 16px;
        background: rgba(30, 30, 30, 0.88);
        color: #fff;
        border-radius: 24px;
        font-size: 14px;
        z-index: 500;
        pointer-events: auto;
        white-space: nowrap;
    }
    .polygon-error-toast button {
        background: #2196f3;
        color: #fff;
        border: none;
        border-radius: 14px;
        padding: 4px 12px;
        font-size: 13px;
        cursor: pointer;
    }

    /* 컨테이너: 하단 고정 & 내부 요소만 클릭 가능하도록 */
.ad-banner-container {
  position: fixed;
  bottom: 0; left: 0; right: 0;
  z-index: 1000;
  pointer-events: none; /* 컨테이너는 이벤트 막고, 내부 요소에서만 허용 */
}

/* 글래스 칩 형태의 대가성 고지 */
.ad-disclaimer-bar {
  position: absolute;
  bottom: 81px; /* 배너 위로 띄우기 (배너 높이에 맞춰 조정) */
  left: 50%;
  transform: translateX(-50%);
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.65);
  backdrop-filter: blur(10px);
  -webkit-backdrop-filter: blur(10px);
  border: 1px solid rgba(255, 255, 255, 0.7);
  box-shadow: 0 4px 14px rgba(0, 0, 0, 0.15);
  font-size: 12px;
  color: #222;
  white-space: nowrap;
  pointer-events: auto;
}

.ad-badge {
  font-weight: 700;
  padding: 2px 7px;
  border-radius: 999px;
  background: linear-gradient(135deg, #ff6b6b, #f06595);
  color: #fff;
  line-height: 1;
}

.ad-sep {
  width: 1px;
  height: 14px;
  background: rgba(0, 0, 0, 0.12);
  border-radius: 1px;
}

/* 배너: 컨테이너 내부의 일반 박스로 수정(기존 position: fixed 제거) */
.ad-banner {
  position: relative;
  bottom: auto; left: auto;
  right: auto; z-index: auto;
  width: 100%;
  color: white;
  padding: 12px 20px;
  text-align: center;
  cursor: pointer;
  font-weight: bold;
  font-size: 14px;
  box-sizing: border-box;
  line-height: 1.5;
  box-shadow: 0 -2px 10px rgba(0, 0, 0, 0.2);
  animation: slideInUp 0.5s ease-out, pulse-glow 3s infinite;
  transition: transform 0.3s ease;
  pointer-events: auto;
}

/* Svelte 스코핑 우회 - 최대 2줄 제한 */
:global(.ad-content) {
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
  line-clamp: 2;
  overflow: hidden;
  word-break: break-word;
  overflow-wrap: break-word;
}

.ad-banner:hover { transform: translateY(-2px); }
.ad-banner:active { transform: translateY(0); }

.ad-banner img {
  max-width: 100%;
  max-height: 40px;
  object-fit: contain;
}

/* 지도 영역 패딩 조금 더 확보(칩 + 배너) */
.map-wrapper.with-ad { padding-bottom: 104px; }


/* 모션 민감 사용자 배려 */
@media (prefers-reduced-motion: reduce) {
  .ad-banner { animation: none; }
}

/* ── 로그인 버튼 (헤더 우측) ─────────────────────────────── */
.header {
    position: relative;
    z-index: 20;
}
.auth-btn {
    position: absolute;
    right: 12px;
    top: 50%;
    transform: translateY(-50%);
    padding: 6px 14px;
    border: none;
    border-radius: 20px;
    background: #4285f4;
    color: white;
    font-size: 13px;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 6px;
    white-space: nowrap;
}
/* 로그인 상태: user-menu-wrap이 위치를 담당하므로 absolute 해제 */
.user-menu-wrap .auth-btn {
    position: static;
    transform: none;
}
.auth-btn.logged-in {
    background: #34a853;
    border-radius: 50%;
    width: 36px;
    height: 36px;
    padding: 0;
    justify-content: center;
}
.user-avatar {
    width: 36px;
    height: 36px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 15px;
    font-weight: bold;
}
.user-menu-wrap {
    position: absolute;
    right: 12px;
    top: 50%;
    transform: translateY(-50%);
}
.user-dropdown {
    position: absolute;
    top: calc(100% + 6px);
    right: 0;
    background: white;
    border-radius: 10px;
    box-shadow: 0 4px 16px rgba(0,0,0,0.15);
    overflow: hidden;
    min-width: 140px;
    z-index: 200;
}
.user-dropdown button {
    width: 100%;
    padding: 12px 16px;
    border: none;
    background: none;
    text-align: left;
    cursor: pointer;
    font-size: 14px;
    color: #333;
    display: flex;
    align-items: center;
    gap: 8px;
}
.user-dropdown button:hover {
    background: #f5f5f5;
}
.user-dropdown .logout-btn {
    color: #e53935;
    border-top: 1px solid #eee;
}

/* ── 저장 위치 FAB ────────────────────────────────────────── */
.save-location-fab {
    position: fixed;
    right: 12px;
    bottom: 140px;
    width: 48px;
    height: 48px;
    border-radius: 50%;
    border: none;
    background: #34a853;
    color: white;
    box-shadow: 0 2px 8px rgba(0,0,0,0.3);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 5;
    transition: all 0.3s;
}
.locations-fab {
    position: fixed;
    right: 12px;
    bottom: 140px; /* 기본값: 나침반 위 (inline style로 덮어씀) */
    width: 48px;
    height: 48px;
    border-radius: 50%;
    border: none;
    background: white;
    color: #555;
    box-shadow: 0 2px 8px rgba(0,0,0,0.3);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 5;
}

/* ── 위치 저장 폼 (지도 위) ──────────────────────────────── */
.save-form-popup {
    position: fixed;
    right: 68px;
    bottom: 140px;
    background: white;
    border-radius: 12px;
    padding: 12px;
    box-shadow: 0 4px 16px rgba(0,0,0,0.2);
    z-index: 10;
    display: flex;
    gap: 8px;
    align-items: center;
    min-width: 220px;
}
.save-form-popup input {
    flex: 1;
    border: 1px solid #ddd;
    border-radius: 6px;
    padding: 6px 10px;
    font-size: 14px;
}
.save-form-popup button {
    padding: 6px 12px;
    border: none;
    border-radius: 6px;
    background: #34a853;
    color: white;
    cursor: pointer;
    font-size: 13px;
}

/* ── 로그인 모달 ──────────────────────────────────────────── */
.modal-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0,0,0,0.5);
    z-index: 100;
    display: flex;
    align-items: center;
    justify-content: center;
}
.modal-box {
    background: white;
    border-radius: 16px;
    padding: 32px 24px;
    width: min(300px, calc(100vw - 40px));
    text-align: center;
    box-shadow: 0 8px 32px rgba(0,0,0,0.2);
    box-sizing: border-box;
}
.modal-box h2 {
    margin: 0 0 8px;
    font-size: 20px;
    color: #333;
}
.modal-box p {
    margin: 0 0 24px;
    font-size: 14px;
    color: #888;
}
.social-btn {
    width: 100%;
    padding: 12px;
    border: none;
    border-radius: 10px;
    font-size: 15px;
    font-weight: 600;
    cursor: pointer;
    margin-bottom: 10px;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
    text-decoration: none;
    box-sizing: border-box;
}
.btn-google { background: #fff; color: #333; border: 1px solid #ddd; }
.btn-naver  { background: #03c75a; color: white; }
.btn-kakao  { background: #fee500; color: #3c1e1e; }
.modal-close {
    margin-top: 8px;
    background: none;
    border: none;
    color: #aaa;
    cursor: pointer;
    font-size: 14px;
}
.privacy-link {
    display: block;
    margin-top: 12px;
    font-size: 12px;
    color: #aaa;
    text-align: center;
    text-decoration: underline;
}

/* ── 저장 위치 패널 ──────────────────────────────────────── */
.locations-panel {
    position: fixed;
    top: 0;
    right: 0;
    bottom: 0;
    width: min(320px, 90vw);
    background: white;
    box-shadow: -4px 0 20px rgba(0,0,0,0.15);
    z-index: 200;
    display: flex;
    flex-direction: column;
    overflow: hidden;
}
.locations-panel-header {
    padding: 16px;
    border-bottom: 1px solid #eee;
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-weight: bold;
    font-size: 16px;
}
.locations-panel-header button {
    background: none;
    border: none;
    font-size: 20px;
    cursor: pointer;
    color: #888;
}
.locations-list {
    flex: 1;
    overflow-y: auto;
    padding: 8px;
}
.location-item {
    border-radius: 10px;
    border: 1px solid #eee;
    margin-bottom: 8px;
    overflow: hidden;
    transition: border-color 0.2s;
}
.location-item.expanded { border-color: #4caf50; }
.location-item-main {
    padding: 12px;
    cursor: pointer;
    display: flex;
    justify-content: space-between;
    align-items: center;
    transition: background 0.2s;
}
.location-item-main:hover { background: #f5f5f5; }
.location-item-actions {
    display: flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
}
.expand-arrow { font-size: 11px; color: #aaa; padding: 4px; }
.location-item-info .name { font-weight: 600; font-size: 14px; }
.location-item-info .date { font-size: 12px; color: #aaa; margin-top: 2px; }
.location-item-delete {
    background: none;
    border: none;
    color: #ff4444;
    cursor: pointer;
    font-size: 16px;
    padding: 2px 6px;
}
.location-detail {
    padding: 10px 12px 12px;
    border-top: 1px solid #f0f0f0;
    background: #fafafa;
}
.location-address {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: #555;
    margin-bottom: 10px;
}
.location-address span { flex: 1; line-height: 1.4; }
.copy-btn {
    background: none;
    border: 1px solid #ddd;
    border-radius: 6px;
    padding: 2px 6px;
    cursor: pointer;
    font-size: 14px;
    flex-shrink: 0;
}
.navi-buttons {
    display: flex;
    gap: 6px;
}
.navi-btn {
    flex: 1;
    padding: 7px 4px;
    border: none;
    border-radius: 8px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    color: white;
}
.navi-btn.kakao  { background: #FEE500; color: #3c1e1e; }
.navi-btn.naver  { background: #03C75A; }
.navi-btn.tmap   { background: #E8330A; }
.locations-empty {
    text-align: center;
    color: #aaa;
    padding: 40px 20px;
    font-size: 14px;
}

</style>

<div class="page-container">
    <header class="header">
        <h1>
            깜장 참나무 지도(안정)
            <span class="engine-switch-badge"><a href="https://cnm2.zam.kr">🧪 새 지도 엔진(MapLibre) 이용해보기</a></span>
        </h1>
        {#if $authUser}
            <div class="user-menu-wrap">
                <button class="auth-btn logged-in" on:click|stopPropagation={() => showUserMenu = !showUserMenu} title={$authUser.nickname}>
                    <div class="user-avatar">{[...$authUser.nickname][0]}</div>
                </button>
                {#if showUserMenu}
                    <div class="user-dropdown">
                        <button on:click={() => { showLocationsPanel = true; showUserMenu = false; }}>
                            📍 저장된 위치
                        </button>
                        <button class="logout-btn" on:click={() => { logout(); showUserMenu = false; }}>
                            🚪 로그아웃
                        </button>
                    </div>
                {/if}
            </div>
        {:else}
            <button class="auth-btn" on:click={() => showLoginModal = true}>
                🔑 로그인
            </button>
        {/if}
    </header>

    <!-- 지도 영역 -->
    <div class="map-wrapper {showAdBanner ? 'with-ad' : ''}">
        <KakaoMap
            bind:this={mapComponent}
            bind:isSatellite
            bind:roadviewMode
            bind:legendHidden={leftControlsHidden}
            bind:overviewHidden={overviewMapHidden}
            controlsReady={menuStateReady}
            legendBottom={(showAdBanner ? 116 : 20) + 56}
            on:searchresults={onSearchResults}
            on:headingupdate={onHeadingUpdate}
            on:headingstop={onHeadingStop}
            on:headingmodechange={onHeadingModeChange}
            on:followchange={onFollowChange}
            on:gpsfixed={() => { isGPSLocating = false; }}
            on:fetcherror={onPolygonFetchError}
            on:polygonsfetched={() => { polygonsReady = true; }}
            on:legendvisibilitychange={onLegendVisibilityChange}
            on:overviewvisibilitychange={onOverviewVisibilityChange}
        />

        <!-- 왼쪽 메뉴: 채집 예보 FAB + KakaoMap 수종 범례 -->
        <button
            class="collecting-fab {showCollectingPanel ? 'active' : ''} {leftControlsHidden ? 'left-controls-hidden' : ''} {menuStateReady ? '' : 'menu-state-restoring'}"
            style="bottom: calc({showAdBanner ? 116 : 20}px + env(safe-area-inset-bottom, 0px));"
            on:touchstart={(event) => handleControlSwipeStart(event, 'left')}
            on:click={() => showCollectingPanel = !showCollectingPanel}
            title="채집 예보"
            aria-label="채집 예보"
        >
            <svg class="control-fab-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <path d="M4 18c0-7.2 4.2-12 11-13 .1 6.9-3.8 11.2-11 13Z"/>
                <path d="M4 18c2.8-3.6 5.5-6.1 9-8"/>
                <circle cx="18" cy="6" r="2.25"/>
                <path d="M18 1.5V1M18 11v-.5M22.5 6H23M13 6h.5M21.2 2.8l.4-.4M14.4 9.6l.4-.4M21.2 9.2l.4.4"/>
            </svg>
        </button>

        <div
            role="group"
            aria-label="오른쪽 지도 메뉴"
            class="right-controls-layer {rightControlsHidden ? 'hidden' : ''} {menuStateReady ? '' : 'menu-state-restoring'}"
            on:touchstart={(event) => handleControlSwipeStart(event, 'right')}
        >
        <span
            class="right-controls-group-bg"
            style="bottom: calc({fabBaseBottom - 6}px + env(safe-area-inset-bottom, 0px));height:{rightControlsGroupHeight}px;"
            aria-hidden="true"
        ></span>

        <!-- 저장 위치 FAB (로그인 + 나침반 활성 시) -->
        {#if $authUser && isHeadingActive}
            {#if showSaveForm}
                <div class="save-form-popup" style="bottom: calc({saveLocationFabBottom}px + env(safe-area-inset-bottom, 0px));">
                    <input
                        bind:value={saveLocationName}
                        placeholder="위치 이름 입력"
                        on:keydown={e => e.key === 'Enter' && handleSaveLocation()}
                        autofocus
                    />
                    <button on:click={handleSaveLocation}>저장</button>
                </div>
            {/if}
            <button class="save-location-fab" style="bottom: calc({saveLocationFabBottom}px + env(safe-area-inset-bottom, 0px));" on:click={() => showSaveForm = !showSaveForm} title="현재 위치 저장">
                <svg width="22" height="22" viewBox="0 0 24 24" fill="currentColor">
                    <path d="M17 3H5a2 2 0 00-2 2v14l7-3 7 3V5a2 2 0 00-2-2z"/>
                </svg>
            </button>
        {/if}

        <!-- 저장 위치 목록 FAB (로그인 시) -->
        {#if $authUser}
            <button class="locations-fab" style="bottom: calc({locationsFabBottom}px + env(safe-area-inset-bottom, 0px));" on:click={() => showLocationsPanel = true} title="저장된 위치">
                <svg width="22" height="22" viewBox="0 0 24 24" fill="currentColor">
                    <path d="M12 2C8.13 2 5 5.13 5 9c0 5.25 7 13 7 13s7-7.75 7-13c0-3.87-3.13-7-7-7zm0 9.5c-1.38 0-2.5-1.12-2.5-2.5s1.12-2.5 2.5-2.5 2.5 1.12 2.5 2.5-1.12 2.5-2.5 2.5z"/>
                </svg>
            </button>
        {/if}

        <!-- 나침반 floating 버튼 -->
        <button
            class="compass-fab {isHeadingActive ? 'active' : ''} {headingMode === 'heading-up' ? 'heading-up' : ''} {isHeadingActive && !isFollowing ? 'paused' : ''} {isGPSLocating ? 'locating' : ''}"
            style="bottom: calc({showAdBanner ? 116 : 20}px + env(safe-area-inset-bottom, 0px));"
            on:click={toggleHeading}
            title={headingButtonTitle}
            aria-label={headingButtonTitle}
        >
            {#if headingMode === 'off'}
            <!-- 꺼짐: 현재 위치 표적 -->
            <svg width="26" height="26" viewBox="0 0 24 24" fill="currentColor" xmlns="http://www.w3.org/2000/svg">
                <path d="M13 1v2.06A9.01 9.01 0 0120.94 11H23v2h-2.06A9.01 9.01 0 0113 20.94V23h-2v-2.06A9.01 9.01 0 013.06 13H1v-2h2.06A9.01 9.01 0 0111 3.06V1h2zm-1 4a7 7 0 100 14 7 7 0 000-14zm0 4a3 3 0 110 6 3 3 0 010-6z"/>
            </svg>
            {:else if headingMode === 'north-up'}
            <!-- 첫 클릭: 지도는 북쪽 고정, 화살표만 바라보는 방향 표시 -->
            <svg width="27" height="27" viewBox="0 0 24 24" fill="currentColor" xmlns="http://www.w3.org/2000/svg">
                <path d="M12 2L4.5 21 12 17.7 19.5 21 12 2zm0 5.8l3.7 9.4-3.7-1.6-3.7 1.6L12 7.8z"/>
            </svg>
            {:else}
            <!-- 두 번째 클릭: 헤딩업, 지도가 바라보는 방향에 맞춰 회전 -->
            <svg width="26" height="26" viewBox="0 0 24 24" fill="currentColor" xmlns="http://www.w3.org/2000/svg">
                <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm0 18c-4.41 0-8-3.59-8-8s3.59-8 8-8 8 3.59 8 8-3.59 8-8 8zm-5.5-2.5l7.51-3.49L17.5 6.5 9.99 9.99 6.5 17.5zm5.5-6.6c.61 0 1.1.49 1.1 1.1s-.49 1.1-1.1 1.1-1.1-.49-1.1-1.1.49-1.1 1.1-1.1z"/>
            </svg>
            {/if}
        </button>

        <!-- 위성지도 토글 버튼 -->
        <button
            class="satellite-fab {isSatellite ? 'active' : ''}"
            style="bottom: calc({showAdBanner ? 116 : 20}px + 56px + env(safe-area-inset-bottom, 0px));"
            on:click={() => mapComponent.toggleSatellite()}
            title={isSatellite ? '일반 지도로 보기' : '위성 지도로 보기'}
            aria-label={isSatellite ? '일반 지도로 보기' : '위성 지도로 보기'}
        >
            <svg class="control-fab-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <path d="m12 3-9 5 9 5 9-5-9-5Z"/>
                <path d="m3 12 9 5 9-5"/>
                <path d="m3 16 9 5 9-5"/>
            </svg>
        </button>

        <!-- 로드뷰(거리뷰) 토글 버튼 -->
        <button
            class="satellite-fab {roadviewMode ? 'active' : ''}"
            style="bottom: calc({showAdBanner ? 116 : 20}px + 112px + env(safe-area-inset-bottom, 0px));"
            on:click={() => mapComponent.toggleRoadview()}
            title={roadviewMode ? '로드뷰 끄기' : '로드뷰 켜기'}
            aria-label={roadviewMode ? '로드뷰 끄기' : '로드뷰 켜기'}
        >
            <svg class="control-fab-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <path d="M2.5 12s3.4-5.5 9.5-5.5 9.5 5.5 9.5 5.5-3.4 5.5-9.5 5.5S2.5 12 2.5 12Z"/>
                <circle cx="12" cy="12" r="2.6"/>
            </svg>
        </button>

        {#if !rightControlsHidden}
            <button
                class="right-controls-swipe-hint"
                style="bottom: calc({rightControlsGripBottom}px + env(safe-area-inset-bottom, 0px));"
                on:click={() => setControlsHidden('right', true)}
                aria-label="오른쪽 메뉴 숨기기"
                title="오른쪽 메뉴 숨기기"
            ></button>
        {/if}

        </div>

        {#if menuStateReady && rightControlsHidden}
            <button
                class="right-controls-edge-tab"
                on:touchstart={(event) => handleControlSwipeStart(event, 'right')}
                on:click={() => setControlsHidden('right', false)}
                title="오른쪽 메뉴 보이기"
                aria-label="오른쪽 메뉴 보이기"
            >◀</button>
        {/if}

        <!-- 채집 예보 패널 -->
        <CollectingForecast
            show={showCollectingPanel}
            lat={forecastLat}
            lng={forecastLng}
            adHeight={showAdBanner ? 104 : 0}
            onclose={() => showCollectingPanel = false}
        />

        <!-- 참나무 지도 로드 실패 토스트 -->
        {#if polygonFetchFailed}
            <div class="polygon-error-toast">
                <span>참나무 지도 로드 실패</span>
                <button on:click={retryPolygons}>재시도</button>
            </div>
        {/if}

        <div
            class="search-container {isHidden ? 'hidden' : ''} {menuStateReady ? '' : 'menu-state-restoring'}"
            bind:this={searchContainer}
        >
            <!-- 드래그 핸들 -->
            <div
                class="drag-handle"
                on:touchstart={handleDragStart}
                on:mousedown={handleDragStart}
            ></div>

            <div class="search-input-box">
                <input type="text" bind:value={searchQuery} placeholder="장소를 검색하세요" on:keydown={handleKeyDown}/>
                <button on:click={handleSearch}>검색</button>
            </div>



            {#if showSearchResults}
                {#if searchResults.length > 0}
                    <ul class="search-results-list">
                        {#each searchResults as result}
                            <li on:click={() => handleResultClick(result)}>
                                <div class="place-name">{result.place_name}</div>
                                <div>{result.address_name}</div>
                            </li>
                        {/each}
                    </ul>
                {:else if hasSearched}
                    <ul class="search-results-list">
                        <li>검색 결과가 없습니다.</li>
                    </ul>
                {/if}
            {/if}
        </div>
    </div>

    <!-- 광고 배너 -->
    {#if showAdBanner && currentAd}
        <div class="ad-banner-container" aria-live="polite">
            <!-- 대가성 고지 칩 -->
            <div class="ad-disclaimer-bar" role="note" aria-label="광고 및 대가성 고지">
            <span class="ad-badge">광고</span>
            <span class="ad-sep" aria-hidden="true"></span>
            <span class="ad-text">쿠팡파트너스 수수료 제공</span>
            </div>

            <!-- 실제 광고 배너(클릭 가능) -->
            <div
            class="ad-banner"
            style="background: {currentAd.backgroundColor}"
            on:click={handleAdBannerClick}
            >
            <div class="ad-content">
                {#if currentAd.type === 'text'}
                {currentAd.content}
                {:else if currentAd.type === 'image'}
                <img src={currentAd.content} alt="광고 배너" />
                {/if}
            </div>
            </div>
        </div>
    {/if}

    <!-- 로그인 모달 -->
    {#if showLoginModal}
        <div class="modal-overlay" on:click|self={() => showLoginModal = false}>
            <div class="modal-box">
                <h2>로그인</h2>
                <p>소셜 계정으로 간편하게 시작하세요</p>
                <a class="social-btn btn-google" href="/api/auth/google">
                    <svg width="18" height="18" viewBox="0 0 48 48">
                        <path fill="#EA4335" d="M24 9.5c3.54 0 6.71 1.22 9.21 3.6l6.85-6.85C35.9 2.38 30.47 0 24 0 14.62 0 6.51 5.38 2.56 13.22l7.98 6.19C12.43 13.72 17.74 9.5 24 9.5z"/>
                        <path fill="#4285F4" d="M46.98 24.55c0-1.57-.15-3.09-.38-4.55H24v9.02h12.94c-.58 2.96-2.26 5.48-4.78 7.18l7.73 6c4.51-4.18 7.09-10.36 7.09-17.65z"/>
                        <path fill="#FBBC05" d="M10.53 28.59c-.48-1.45-.76-2.99-.76-4.59s.27-3.14.76-4.59l-7.98-6.19C.92 16.46 0 20.12 0 24c0 3.88.92 7.54 2.56 10.78l7.97-6.19z"/>
                        <path fill="#34A853" d="M24 48c6.48 0 11.93-2.13 15.89-5.81l-7.73-6c-2.18 1.48-4.97 2.35-8.16 2.35-6.26 0-11.57-4.22-13.47-9.91l-7.98 6.19C6.51 42.62 14.62 48 24 48z"/>
                    </svg>
                    Google로 로그인
                </a>
                <a class="social-btn btn-naver" href="/api/auth/naver">
                    <svg width="18" height="18" viewBox="0 0 24 24" fill="white">
                        <path d="M16.273 12.845L7.376 0H0v24h7.727V11.155L16.624 24H24V0h-7.727z"/>
                    </svg>
                    네이버로 로그인
                </a>
                <a class="social-btn btn-kakao" href="/api/auth/kakao">
                    <svg width="18" height="18" viewBox="0 0 24 24" fill="#3c1e1e">
                        <path d="M12 3C6.48 3 2 6.48 2 10.8c0 2.7 1.47 5.07 3.7 6.5L4.5 21l4.13-2.19c1.1.3 2.24.47 3.37.47 5.52 0 10-3.48 10-7.8S17.52 3 12 3z"/>
                    </svg>
                    카카오로 로그인
                </a>
                <button class="modal-close" on:click={() => showLoginModal = false}>닫기</button>
                <a href="/privacy" class="privacy-link" target="_blank">개인정보처리방침</a>
            </div>
        </div>
    {/if}

    <!-- 저장 위치 패널 -->
    {#if showLocationsPanel}
        <div class="locations-panel">
            <div class="locations-panel-header">
                <span>저장된 위치</span>
                <button on:click={() => showLocationsPanel = false}>✕</button>
            </div>
            <div class="locations-list">
                {#if savedLocations.length === 0}
                    <div class="locations-empty">
                        저장된 위치가 없습니다.<br>
                        나침반 모드에서 북마크 버튼으로 저장하세요.
                    </div>
                {:else}
                    {#each savedLocations as loc (loc.id)}
                        <div class="location-item {expandedLocationId === loc.id ? 'expanded' : ''}">
                            <div class="location-item-main" on:click={() => { handleLocationMarkerClick(loc); toggleLocationExpand(loc); }}>
                                <div class="location-item-info">
                                    <div class="name">{loc.name}</div>
                                    <div class="date">{loc.created_at}</div>
                                </div>
                                <div class="location-item-actions">
                                    <span class="expand-arrow">{expandedLocationId === loc.id ? '▲' : '▼'}</span>
                                    <button
                                        class="location-item-delete"
                                        on:click|stopPropagation={() => handleDeleteLocation(loc.id)}
                                        title="삭제"
                                    >🗑</button>
                                </div>
                            </div>
                            {#if expandedLocationId === loc.id}
                                <div class="location-detail">
                                    <div class="location-address">
                                        <span>{locationAddresses[loc.id] ?? '...'}</span>
                                        <button class="copy-btn" on:click|stopPropagation={() => copyAddress(loc)} title="주소 복사">📋</button>
                                    </div>
                                    <div class="navi-buttons">
                                        <button class="navi-btn kakao" on:click|stopPropagation={() => openKakaoNavi(loc)}>
                                            카카오맵
                                        </button>
                                        <button class="navi-btn naver" on:click|stopPropagation={() => openNaverNavi(loc)}>
                                            네이버
                                        </button>
                                        <button class="navi-btn tmap" on:click|stopPropagation={() => openTmap(loc)}>
                                            티맵
                                        </button>
                                    </div>
                                </div>
                            {/if}
                        </div>
                    {/each}
                {/if}
            </div>
        </div>
    {/if}
</div>

