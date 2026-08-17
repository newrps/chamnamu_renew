import { writable } from 'svelte/store';

export interface AuthUser {
    id: number;
    nickname: string;
}

export interface SavedLocation {
    id: number;
    user_id: number;
    name: string;
    lat: number;
    lng: number;
    memo: string | null;
    created_at: string;
}

export const authUser = writable<AuthUser | null>(null);

// 로그인 상태는 서버가 심어주는 httpOnly 쿠키로만 유지함 - JS에서 토큰 값을 직접
// 다루지 않으므로 XSS가 터져도 토큰을 빼돌려 다른 곳에서 재사용할 수 없음.
// 로그인 여부/사용자 정보는 /api/me 호출 결과로만 판단함.
export async function initAuth() {
    // httpOnly 쿠키 방식 전환 전에 저장된 기존 JWT를 사용자 기기에서 제거한다.
    try {
        localStorage.removeItem('jwt');
    } catch {
        // 저장소 접근이 차단된 브라우저에서도 인증 초기화는 계속 진행한다.
    }

    try {
        const res = await fetch('/api/me');
        if (!res.ok) {
            authUser.set(null);
            return;
        }
        const me = await res.json();
        authUser.set({ id: me.id, nickname: me.nickname });
    } catch {
        authUser.set(null);
    }
}

export async function logout() {
    try {
        await fetch('/api/auth/logout', { method: 'POST' });
    } catch {
        // 네트워크 실패해도 클라이언트 쪽 로그인 상태는 정리함
    }
    authUser.set(null);
}

function jsonHeaders(): HeadersInit {
    return { 'Content-Type': 'application/json' };
}

// ── 저장 위치 API ─────────────────────────────────────────────────────────────
// 인증 쿠키는 같은 출처(same-origin) 요청이라 브라우저가 자동으로 실어 보내줌

export async function fetchLocations(): Promise<SavedLocation[]> {
    const res = await fetch('/api/locations');
    if (!res.ok) return [];
    return res.json();
}

export async function saveLocation(name: string, lat: number, lng: number, memo?: string): Promise<SavedLocation | null> {
    const res = await fetch('/api/locations', {
        method: 'POST',
        headers: jsonHeaders(),
        body: JSON.stringify({ name, lat, lng, memo }),
    });
    if (!res.ok) return null;
    return res.json();
}

export async function deleteLocation(id: number): Promise<boolean> {
    const res = await fetch(`/api/locations/${id}`, {
        method: 'DELETE',
    });
    return res.ok;
}
