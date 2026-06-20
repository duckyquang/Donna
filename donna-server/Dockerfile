# Build stage
FROM rust:1.77-slim-bookworm AS builder
WORKDIR /app
COPY Cargo.toml .
COPY src/ src/
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/donna-server /usr/local/bin/donna-server
CMD ["donna-server"]
