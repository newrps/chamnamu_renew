/// <reference types="@sveltejs/kit" />

// 카카오맵 API의 타입을 정의하는 부분
declare namespace kakao {
    namespace maps {
        class LatLng {
            constructor(lat: number, lng: number);
            getLat(): number;
            getLng(): number;
            equals(other: LatLng): boolean;
            toString(): string;
        }

        class Map {
            constructor(container: HTMLElement, options: object);
            setCenter(latlng: LatLng): void;
            getCenter(): LatLng;
            setBounds(bounds: LatLngBounds): void;
            getBounds(): LatLngBounds;
            getLevel(): number;
            setLevel(level: number): void;
            getMapTypeId(): MapTypeId;
            setMapTypeId(mapTypeId: MapTypeId): void;
            setDraggable(draggable: boolean): void;
            getDraggable(): boolean;
            addControl(control: any, position: any): void;
            removeControl(control: any): void;
        }

        class Marker {
            constructor(options: { position: LatLng, map: Map | null });
            setMap(map: Map | null): void;
        }

        class InfoWindow {
            constructor(options: { content?: string, zIndex?: number });
            setContent(content: string): void;
            open(map: Map, marker: Marker): void;
            close(): void;
        }

        class LatLngBounds {
            constructor(sw?: LatLng, ne?: LatLng);
            extend(latlng: LatLng): void;
            getCenter(): LatLng;
        }
        
        class Polygon {
            constructor(options: { path: LatLng[], strokeWeight: number, strokeColor: string, strokeOpacity: number, fillColor: string, fillOpacity: number });
            setMap(map: Map | null): void;
        }

        namespace event {
            function addListener(target: any, type: string, handler: (e: any) => void): void;
        }

        enum MapTypeId {
            NORMAL = "NORMAL",
            ROADVIEW = "ROADVIEW"
        }
    }

    namespace maps.services {
        class Places {
            constructor();
            keywordSearch(
                keyword: string,
                callback: (data: any, status: string) => void
            ): void;
        }
        // Status 객체를 Places 클래스 밖, services 네임스페이스 안에 정의합니다.
        const Status: {
            OK: string;
            ZERO_RESULT: string;
            ERROR: string;
        };
    }
}
