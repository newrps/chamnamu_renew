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
    function overviewProject(lng: number, lat: number) {
        const { minLng, maxLng, minLat, maxLat, width, height, padding } = OVERVIEW_GEO;
        const clampedLng = Math.max(minLng, Math.min(maxLng, lng));
        const clampedLat = Math.max(minLat, Math.min(maxLat, lat));
        return {
            x: padding + ((clampedLng - minLng) / (maxLng - minLng)) * (width - padding * 2),
            y: padding + ((maxLat - clampedLat) / (maxLat - minLat)) * (height - padding * 2)
        };
    }

    // 통계청 기반 시·도 경계를 미니맵 해상도에 맞게 단순화한 정적 SVG 경로
    const OVERVIEW_REGIONS = [
        {"name":"제주특별자치도","d":"M43.8,135.2L45.8,136.2L46.4,137.7L44.7,141.5L40.9,143.3L36.5,143.3L35.8,144.4L34.0,141.9L35.6,138.4L39.3,136.3L43.8,135.2Z"},
        {"name":"경상남도","d":"M63.7,101.8L64.2,104.0L62.7,103.7L63.7,101.8ZM61.5,101.1L62.5,104.6L64.2,104.2L63.6,106.7L62.5,107.0L62.4,105.3L61.4,106.7L60.6,106.0L60.1,103.3L61.5,101.1ZM74.0,98.8L74.7,99.4L74.3,102.5L74.9,102.5L74.4,103.7L75.0,104.9L73.9,104.4L73.3,105.7L74.1,106.3L72.9,107.2L71.8,105.0L72.5,104.7L72.4,103.4L71.4,104.5L70.7,102.8L71.6,101.7L72.9,102.2L72.5,101.1L73.5,100.7L74.0,98.8ZM69.1,84.7L71.2,84.0L71.6,83.0L72.9,85.5L75.7,85.8L77.7,84.1L79.1,84.5L79.0,86.8L82.6,89.7L79.1,94.0L75.8,95.9L76.6,97.2L75.9,97.6L74.4,97.2L74.1,96.1L73.0,96.2L72.5,94.6L73.3,98.1L72.4,98.5L71.7,97.0L69.3,98.6L70.9,98.2L71.2,99.5L70.2,100.0L69.9,102.4L70.4,101.9L70.7,102.6L69.7,105.7L68.6,104.0L69.5,103.2L68.0,102.2L68.9,102.1L68.5,101.2L67.7,102.2L67.0,101.2L66.3,101.4L66.4,102.4L65.1,102.2L63.7,100.9L63.6,99.0L62.9,100.4L62.1,99.5L61.7,100.8L59.8,101.3L59.5,98.6L57.2,95.2L56.5,92.2L57.8,89.5L56.7,85.9L57.8,81.1L61.4,77.2L62.1,78.7L65.2,79.5L66.4,82.0L65.8,83.7L68.1,83.6L69.1,84.7Z"},
        {"name":"경상북도","d":"M86.2,83.8L84.7,83.0L83.2,83.7L83.1,82.5L81.4,81.9L80.3,84.0L78.8,84.8L77.7,84.1L75.7,85.8L72.5,85.3L71.7,84.4L71.8,82.1L74.1,82.0L75.3,78.0L74.2,74.8L71.6,75.7L71.0,77.7L70.8,76.7L69.9,77.1L69.4,78.8L70.8,79.4L69.3,81.0L70.1,82.5L68.8,82.5L69.3,84.7L68.1,83.6L65.8,83.7L66.4,82.0L65.2,79.5L63.4,79.4L61.4,77.8L61.3,74.6L62.6,73.5L62.8,70.5L64.0,70.2L64.0,68.8L60.8,68.3L61.7,62.6L60.0,60.6L61.2,59.0L62.1,59.9L61.4,58.1L62.6,57.0L64.3,57.7L64.1,55.0L66.7,55.0L67.1,53.6L68.3,55.0L69.9,55.1L70.2,52.2L73.6,48.9L75.2,49.8L75.7,48.3L77.4,49.4L77.9,48.2L80.1,48.8L80.6,48.0L82.8,49.4L83.4,47.6L84.9,46.9L85.9,48.9L85.7,53.2L86.7,57.9L85.6,59.8L86.2,62.7L85.1,68.0L86.1,72.7L85.1,73.9L86.3,75.3L87.8,73.0L88.4,74.7L86.2,83.8ZM109.4,36.9L109.5,38.6L108.2,39.0L107.5,37.8L109.4,36.9Z"},
        {"name":"전라남도","d":"M40.2,120.1L40.6,120.9L39.8,121.5L40.2,120.1ZM41.9,119.7L42.0,121.7L41.4,120.9L41.9,119.7ZM45.6,119.1L46.1,120.2L45.5,120.8L44.9,120.2L45.6,119.1ZM40.5,118.7L41.4,119.5L40.8,120.2L40.5,118.7ZM32.1,116.6L32.9,117.6L31.9,117.5L32.1,116.6ZM44.8,115.8L45.7,116.3L44.9,117.1L43.8,116.4L44.8,115.8ZM47.9,115.2L48.9,116.2L48.1,116.4L47.9,115.2ZM42.3,114.6L43.5,117.3L41.8,116.3L42.3,114.6ZM46.4,114.3L46.0,115.7L45.2,115.3L46.4,114.3ZM44.7,113.6L45.2,114.9L43.7,115.7L43.6,114.5L44.7,113.6ZM54.7,112.8L55.9,113.4L55.2,114.3L54.7,112.8ZM50.9,112.3L50.7,113.8L49.3,113.8L49.3,112.4L50.9,112.3ZM54.6,111.0L55.3,112.3L54.5,112.7L54.6,111.0ZM59.1,110.8L60.0,112.3L58.8,111.8L59.1,110.8ZM35.4,109.9L37.5,112.0L37.2,113.6L34.2,115.8L32.9,114.2L35.5,111.2L35.4,109.9ZM32.5,108.8L33.2,109.9L32.2,111.4L32.5,108.8ZM32.2,108.2L32.5,110.1L31.7,110.6L32.2,108.2ZM33.8,107.9L34.0,109.3L33.2,108.5L33.8,107.9ZM22.6,107.2L22.1,109.1L21.7,108.6L22.6,107.2ZM59.2,106.4L59.8,110.0L58.6,109.1L59.2,106.4ZM30.9,106.5L31.7,107.0L30.9,108.3L30.1,107.0L30.9,106.5ZM32.7,105.3L34.5,106.4L33.8,107.5L32.5,106.8L32.7,105.3ZM31.4,104.6L31.4,105.7L30.2,106.9L29.7,105.6L31.4,104.6ZM34.1,104.7L34.2,105.7L33.0,105.3L34.1,104.7ZM33.3,102.7L33.8,103.6L33.0,104.8L32.4,103.5L33.3,102.7ZM36.2,101.7L37.1,104.4L36.8,103.6L35.1,103.5L36.2,101.7ZM32.4,101.4L33.1,102.2L32.2,103.4L31.3,102.8L32.4,101.4ZM59.0,101.1L59.8,102.6L58.5,102.0L59.0,101.1ZM33.6,99.1L34.4,100.1L34.0,101.3L33.0,99.8L33.6,99.1ZM34.0,96.3L33.3,98.3L32.2,98.3L32.2,97.3L34.0,96.3ZM56.5,92.2L57.2,95.2L59.8,99.4L57.5,102.6L56.7,101.8L57.5,104.0L59.6,103.3L59.2,106.3L58.5,106.7L57.8,105.8L57.5,108.5L56.4,108.6L56.7,105.8L55.3,102.9L53.7,104.1L53.3,106.2L54.9,107.9L55.4,109.9L53.6,110.1L54.2,110.9L52.7,113.5L51.4,111.7L49.1,111.0L51.3,107.0L52.3,108.1L52.5,105.9L51.5,106.4L51.1,105.5L50.2,107.4L49.4,107.2L47.2,109.3L47.0,112.5L46.3,113.4L45.4,113.2L45.5,113.9L44.0,113.0L43.7,109.6L43.0,113.5L41.3,114.6L41.0,116.5L39.7,117.1L38.8,111.4L36.4,110.5L35.5,108.1L36.2,105.7L38.3,109.9L36.9,107.6L37.5,105.6L38.6,105.2L37.2,105.0L37.7,100.8L36.9,101.9L36.1,100.7L37.5,98.6L36.7,97.6L35.5,99.5L34.0,98.2L34.1,97.2L36.9,96.1L36.8,97.5L37.7,97.9L37.8,99.0L38.5,98.4L38.8,97.5L36.8,95.1L36.3,95.4L36.2,94.2L38.1,89.2L39.0,89.3L39.9,92.2L42.5,91.2L43.3,88.7L44.6,87.8L46.3,90.1L47.9,88.3L48.1,92.0L49.2,92.4L50.9,91.5L53.4,92.3L54.8,90.8L56.5,92.2Z"},
        {"name":"전라북도","d":"M57.5,73.5L59.8,74.8L60.9,74.2L61.8,76.6L57.8,81.1L56.7,85.9L57.9,88.8L56.7,92.1L54.8,90.8L53.4,92.3L50.9,91.5L49.2,92.4L48.1,92.0L47.9,88.3L46.3,90.1L44.6,87.8L43.3,88.7L42.5,91.2L39.9,92.2L39.2,89.7L38.3,89.0L39.1,87.0L42.1,85.5L39.3,85.4L38.8,84.2L41.0,82.1L41.3,80.4L43.7,80.6L42.4,78.8L44.2,77.8L41.2,78.0L41.2,76.6L39.8,76.6L39.7,75.9L43.0,75.3L46.6,71.3L48.1,71.6L48.3,72.8L49.5,73.4L52.7,71.9L53.3,74.3L55.1,75.7L55.9,74.3L57.1,75.0L57.5,73.5Z"},
        {"name":"충청남도","d":"M37.2,60.0L38.1,65.1L36.6,63.6L36.4,60.7L37.2,60.0ZM45.9,52.8L49.6,51.2L53.7,55.5L53.9,56.5L52.7,56.6L51.9,58.1L49.9,57.0L49.5,57.7L50.4,60.8L50.0,62.5L50.5,64.2L51.8,64.9L51.3,68.1L52.7,70.5L53.2,68.4L54.4,70.4L55.8,69.0L56.8,69.8L57.1,75.0L55.9,74.3L55.8,75.3L54.6,75.5L53.3,74.3L52.7,71.9L49.5,73.4L48.3,72.8L48.1,71.6L46.6,71.3L43.1,75.1L41.9,74.9L41.5,72.8L39.6,71.4L40.1,68.6L39.4,67.1L40.2,66.6L39.1,65.7L38.8,58.7L38.2,60.3L36.9,58.7L37.1,59.7L36.1,60.7L35.7,57.2L33.5,58.7L35.0,57.3L34.1,56.5L33.6,57.4L33.1,56.1L34.8,52.9L35.4,52.7L35.3,53.8L35.7,53.3L36.3,51.1L35.9,55.0L36.8,55.0L36.6,54.0L37.9,52.8L36.6,50.6L38.2,50.1L39.0,51.2L38.6,49.8L39.4,48.9L43.8,51.1L44.7,53.9L45.9,52.8Z"},
        {"name":"충청북도","d":"M73.6,48.9L70.2,52.2L69.9,55.1L68.3,55.0L67.1,53.6L66.7,55.0L64.1,55.0L64.3,57.7L62.6,57.0L61.4,58.1L62.1,59.9L61.2,59.0L60.0,60.6L61.7,62.6L60.8,68.3L61.5,67.9L62.7,69.0L63.4,68.4L64.0,70.2L63.0,70.2L62.6,73.5L61.3,74.6L59.8,74.8L58.2,73.6L57.8,74.2L56.7,71.8L56.8,69.8L55.0,68.7L55.9,64.8L53.7,63.9L53.7,61.8L51.8,59.5L52.7,56.6L53.9,56.5L52.2,53.8L52.2,52.2L54.6,49.4L56.3,49.3L59.2,45.1L59.9,46.9L61.7,46.7L63.0,44.1L63.8,45.8L66.4,44.3L67.5,45.3L68.2,45.0L67.6,46.8L69.6,46.8L69.9,47.7L73.6,48.9Z"},
        {"name":"강원도","d":"M84.9,46.9L83.4,47.6L82.8,49.4L80.6,48.0L80.1,48.8L77.9,48.2L77.4,49.4L75.7,48.3L75.2,49.8L71.3,47.7L69.9,47.7L69.6,46.8L67.9,47.1L67.4,46.5L68.5,45.2L66.4,44.3L63.8,45.8L63.0,44.1L61.7,46.7L59.9,46.9L59.1,45.3L60.1,39.9L59.6,38.0L60.8,36.8L56.9,34.4L55.9,34.6L55.7,30.2L57.1,27.3L54.6,25.4L54.2,23.0L52.4,23.5L51.9,21.4L50.3,21.8L49.9,20.1L49.1,19.8L49.6,17.7L53.5,17.4L55.4,18.3L59.7,17.1L60.1,17.8L64.2,18.1L67.7,15.1L68.8,10.7L72.9,22.1L80.0,33.8L81.0,37.6L84.8,44.7L84.9,46.9Z"},
        {"name":"경기도","d":"M41.1,42.8L40.5,43.6L41.7,45.3L40.1,45.4L40.2,43.6L41.1,42.8ZM59.2,45.1L56.3,49.3L54.6,49.4L51.9,53.1L49.1,51.1L45.9,52.8L43.4,49.7L44.0,46.1L42.4,47.6L41.9,46.5L42.4,44.0L44.0,44.4L44.6,43.3L43.0,43.0L42.4,41.9L43.8,39.3L43.6,36.8L45.8,39.8L47.4,38.9L48.2,39.9L49.7,38.7L50.2,36.6L48.9,36.7L48.6,33.3L47.6,33.2L47.1,34.8L45.9,34.6L45.0,36.3L41.9,34.7L40.5,36.0L39.8,34.4L39.6,31.6L41.1,31.9L42.0,31.0L42.1,26.9L43.3,27.0L45.6,24.5L43.3,22.6L45.3,20.3L46.8,20.9L49.1,18.6L50.3,21.8L51.9,21.4L52.4,23.5L54.2,23.0L54.6,25.4L56.9,26.9L56.9,28.9L55.7,30.2L55.9,34.6L56.9,34.4L60.8,36.8L59.6,38.0L60.1,39.9L59.2,45.1Z"},
        {"name":"세종특별자치시","d":"M51.9,58.1L52.2,60.7L53.7,61.8L53.8,62.9L51.4,65.1L50.0,62.5L50.4,60.8L49.5,57.7L49.9,57.0L51.9,58.1Z"},
        {"name":"울산광역시","d":"M86.2,83.8L86.1,87.8L84.7,88.8L84.8,91.0L83.9,91.7L83.3,90.3L82.3,90.2L82.2,89.0L78.6,86.0L81.4,81.9L83.1,82.5L83.2,83.7L84.7,83.0L86.2,83.8Z"},
        {"name":"대전광역시","d":"M55.2,69.3L54.4,70.4L53.2,68.4L52.7,70.5L51.3,68.1L51.8,64.9L53.4,62.8L53.7,63.9L55.3,64.1L55.3,65.1L56.1,65.3L55.2,69.3Z"},
        {"name":"광주광역시","d":"M45.9,93.4L47.7,95.7L47.5,96.7L46.6,98.0L43.8,98.5L43.2,97.2L41.8,96.8L41.9,95.1L43.1,93.6L44.2,94.4L45.9,93.4Z"},
        {"name":"인천광역시","d":"M33.2,43.7L34.0,44.7L33.3,45.3L33.2,43.7ZM38.9,43.4L39.4,44.1L38.7,44.9L38.9,43.4ZM40.6,38.1L38.5,40.1L37.1,38.9L39.4,37.4L40.6,38.1ZM44.1,36.1L43.3,37.8L43.7,39.9L41.2,42.0L40.5,36.0L41.9,34.7L44.1,36.1ZM36.7,32.0L37.4,34.0L36.6,34.6L35.7,33.0L36.7,32.0ZM35.7,30.3L36.8,30.8L35.0,31.9L35.7,30.3ZM39.8,32.3L40.1,35.5L39.6,36.2L37.9,35.8L37.0,30.9L38.4,30.0L39.8,32.3ZM10.6,26.3L11.3,26.9L10.7,27.9L9.8,27.7L9.4,26.4L10.6,26.3Z"},
        {"name":"대구광역시","d":"M71.6,83.0L71.2,84.0L69.1,84.7L69.6,84.0L68.8,82.5L70.1,82.5L69.3,81.0L70.8,79.4L69.3,78.5L70.2,76.7L71.0,77.7L71.6,75.7L74.2,74.8L75.3,78.0L74.1,80.3L74.2,81.7L73.1,82.5L72.4,81.6L71.6,83.0Z"},
        {"name":"부산광역시","d":"M83.9,91.7L81.0,97.3L79.9,96.8L80.4,98.6L79.6,97.7L78.6,99.0L78.3,97.9L76.5,97.8L76.7,99.0L76.0,99.3L76.6,97.2L75.8,95.9L79.1,94.0L82.3,90.2L83.3,90.3L83.9,91.7Z"},
        {"name":"서울특별시","d":"M43.6,36.8L44.2,35.6L45.4,36.2L46.5,34.2L46.9,34.9L47.6,33.2L48.8,33.5L48.9,36.7L50.2,36.6L49.7,38.7L48.2,39.9L47.4,38.9L45.8,39.8L43.6,36.8Z"},
    ];

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
                    <g fill="#d7e7ce" stroke="#6f9270" stroke-width="0.7" stroke-linejoin="round" filter="url(#overview-shadow)">
                        {#each OVERVIEW_REGIONS as region}
                            <path d={region.d}>
                                <title>{region.name}</title>
                            </path>
                        {/each}
                    </g>
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
