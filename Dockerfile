# Multi-stage Docker build
FROM rust:1.81-slim-bookworm AS builder

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY src/ src/
COPY static/ static/
RUN cargo build --release --locked

# Stage 2: runtime (tiny) with pre-built graph
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /data
COPY --from=builder /app/target/release/scooter-routing /usr/local/bin/
# Use pre-built graph from local build context
COPY region.graph /data/region.graph

EXPOSE 3000

CMD ["scooter-routing", "serve", "--graph", "region.graph", "--bind", "0.0.0.0:3000"]
