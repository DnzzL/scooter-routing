# Multi-stage Docker build
FROM rust:1.94-slim-bookworm AS builder

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY src/ src/
COPY static/ static/
RUN cargo build --release --locked

# Stage 2: import OSM data + build graph
FROM debian:bookworm-slim AS importer

RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*
WORKDIR /data
COPY --from=builder /app/target/release/scooter-routing /usr/local/bin/

# Download PACA OSM extract (~366MB) and build compact graph (~153MB)
RUN curl -L -o region.osm.pbf "https://download.geofabrik.de/europe/france/provence-alpes-cote-d-azur-latest.osm.pbf" && \
    scooter-routing import --output region.graph region.osm.pbf && \
    rm region.osm.pbf

# Stage 3: runtime (tiny)
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /data
COPY --from=builder /app/target/release/scooter-routing /usr/local/bin/
COPY --from=importer /data/region.graph /data/region.graph
COPY static/ /data/static/

EXPOSE 3000

CMD ["scooter-routing", "serve", "--graph", "region.graph", "--bind", "0.0.0.0:3000"]
