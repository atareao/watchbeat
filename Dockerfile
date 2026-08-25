# ═══════════════════════════════════════════════════════════════
# Stage 1: Frontend (npm)
# ═══════════════════════════════════════════════════════════════
FROM docker.io/library/node:23-alpine AS frontend-builder

WORKDIR /build
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci

COPY frontend/ ./
RUN npm run build

# ═══════════════════════════════════════════════════════════════
# Stage 2: Backend (Rust)
# ═══════════════════════════════════════════════════════════════
FROM docker.io/library/rust:alpine3.23 AS backend-builder

RUN apk add --no-cache --update \
    build-base \
    musl-dev \
    pkgconfig \
    openssl-dev \
    openssl-libs-static

WORKDIR /build

# Cache dependencies (avoid recompiling every time)
RUN cargo init --bin --name watchbeat . && \
    mkdir -p src && \
    echo "// dummy" > src/lib.rs

COPY backend/Cargo.toml backend/Cargo.lock ./
RUN cargo build --release && \
    rm -rf src

# Copy frontend dist first so include_dir! macro can find it
COPY --from=frontend-builder /build/dist /build/../frontend/dist

COPY backend/src ./src
RUN find src -type f -exec touch {} + && \
    cargo build --release && \
    strip target/release/watchbeat

# ═══════════════════════════════════════════════════════════════
# Stage 3: Runtime
# ═══════════════════════════════════════════════════════════════
FROM alpine:3.23

RUN apk add --no-cache \
    ca-certificates \
    && adduser -D -h /app -u 1000 app

WORKDIR /app
COPY --from=backend-builder /build/target/release/watchbeat .
COPY --from=frontend-builder /build/dist ./dist

RUN mkdir -p /app/data && chown -R app:app /app

USER app
EXPOSE 3055
CMD ["./watchbeat"]