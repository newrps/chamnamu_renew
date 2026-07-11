# 1단계: 빌드
FROM node:20-alpine AS builder
WORKDIR /app

ARG VITE_API_BASE_URL=""
ARG VITE_KAKAO_MAP_API_KEY

RUN npm install -g pnpm

COPY package.json pnpm-lock.yaml ./
RUN pnpm install --no-frozen-lockfile

COPY . .
# Vite가 build 시 명시적으로 .env.local 을 읽도록 ARG → 파일로 작성
RUN printf 'VITE_API_BASE_URL=%s\nVITE_KAKAO_MAP_API_KEY=%s\n' "$VITE_API_BASE_URL" "$VITE_KAKAO_MAP_API_KEY" > .env.local
RUN pnpm build

# 2단계: nginx로 정적 파일 서빙
FROM nginx:alpine
COPY --from=builder /app/build /usr/share/nginx/html
COPY nginx.conf /etc/nginx/conf.d/default.conf

EXPOSE 80
