# syntax=docker/dockerfile:1

FROM node:22-alpine AS frontend-build
WORKDIR /app/frontend
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/ ./
RUN npm run build

FROM rust:1.90-slim AS backend-build
WORKDIR /app
COPY Cargo.toml ./
COPY backend/ backend/
ENV SQLX_OFFLINE=true
RUN cargo build --release --manifest-path backend/Cargo.toml

FROM debian:bookworm-slim AS final
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=backend-build /app/target/release/benefrance-backend ./benefrance-backend
COPY --from=frontend-build /app/frontend/build ./static
ENV STATIC_DIR=/app/static
EXPOSE 3000
ENTRYPOINT ["./benefrance-backend"]
