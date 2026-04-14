import { writable, get } from 'svelte/store';

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

// JWT 디코딩 (검증 없이 페이로드만) - UTF-8 한글 지원
function decodeJwt(token: string): any {
    try {
        const base64 = token.split('.')[1]
            .replace(/-/g, '+')
            .replace(/_/g, '/');
        const json = decodeURIComponent(
            atob(base64).split('').map(c =>
                '%' + ('00' + c.charCodeAt(0).toString(16)).slice(-2)
            ).join('')
        );
        return JSON.parse(json);
    } catch {
        return null;
    }
}

// 앱 시작 시 호출 - URL 토큰 처리 & localStorage 복원
export function initAuth() {
    // OAuth 콜백 후 URL에 token이 붙어오는 경우
    const params = new URLSearchParams(window.location.search);
    const token = params.get('token');
    if (token) {
        localStorage.setItem('jwt', token);
        window.history.replaceState({}, '', '/');
    }

    const stored = localStorage.getItem('jwt');
    if (!stored) return;

    const payload = decodeJwt(stored);
    if (!payload) { localStorage.removeItem('jwt'); return; }
    if (payload.exp * 1000 < Date.now()) { localStorage.removeItem('jwt'); return; }

    authUser.set({ id: payload.sub, nickname: payload.nickname });
}

export function logout() {
    localStorage.removeItem('jwt');
    authUser.set(null);
}

export function getToken(): string | null {
    return localStorage.getItem('jwt');
}

function authHeaders(): HeadersInit {
    const token = getToken();
    return token ? { 'Authorization': `Bearer ${token}`, 'Content-Type': 'application/json' } : {};
}

// ── 저장 위치 API ─────────────────────────────────────────────────────────────

export async function fetchLocations(): Promise<SavedLocation[]> {
    const res = await fetch('/api/locations', { headers: authHeaders() });
    if (!res.ok) return [];
    return res.json();
}

export async function saveLocation(name: string, lat: number, lng: number, memo?: string): Promise<SavedLocation | null> {
    const res = await fetch('/api/locations', {
        method: 'POST',
        headers: authHeaders(),
        body: JSON.stringify({ name, lat, lng, memo }),
    });
    if (!res.ok) return null;
    return res.json();
}

export async function deleteLocation(id: number): Promise<boolean> {
    const res = await fetch(`/api/locations/${id}`, {
        method: 'DELETE',
        headers: authHeaders(),
    });
    return res.ok;
}
