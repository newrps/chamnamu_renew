# 1단계: 빌드 (Rust 공식 이미지)
FROM rust:1.88-slim AS builder
WORKDIR /app

# 의존성 캐싱을 위해 Cargo 파일만 먼저 복사
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf src

# 실제 소스 복사 후 빌드
COPY src ./src
RUN touch src/main.rs && cargo build --release

# 2단계: 최소 런타임 이미지
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/map_api /usr/local/bin/map_api

EXPOSE 8080
CMD ["map_api"]
