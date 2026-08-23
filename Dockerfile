# Build stage
FROM rust:1.85-slim-bookworm AS backend-builder
WORKDIR /app
COPY backend/ .
RUN cargo build --release

# Node build stage
FROM node:22-slim AS frontend-builder
WORKDIR /app
COPY frontend/ .
RUN npm install && npm run build

# Final stage
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates iputils-ping && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=backend-builder /app/target/release/watchbeat /app/watchbeat
COPY --from=frontend-builder /app/dist /app/frontend/dist
EXPOSE 3055
CMD ["/app/watchbeat"]