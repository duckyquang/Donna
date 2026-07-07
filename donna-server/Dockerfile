# Workspace-aware build: this repo's Cargo.toml is a workspace with three members
# (src-tauri, donna-server, crates/donna-core), but src-tauri needs the Tauri/system
# toolchain we don't want in a server image. We stub src-tauri down to just its
# Cargo.toml + empty lib/main so cargo can resolve the workspace graph, then build
# only donna-server (`-p donna-server`) — src-tauri is never a dependency of
# donna-server, so cargo never compiles its stubbed sources or runs its build script.
#
# ponytail: base image is 1.86 not the brief's 1.77 — Cargo.lock has since picked up
# icu_*/idna_adapter transitives (via url/reqwest) that need rustc >=1.86 to parse.
# Matches the local dev toolchain (1.92) is safest going forward; bump alongside
# `rustc --version` if a future dependency raises the floor again.
FROM rust:1.86-slim-bookworm AS build
WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY donna-server ./donna-server
COPY src-tauri/Cargo.toml ./src-tauri/Cargo.toml
RUN mkdir -p src-tauri/src \
 && echo "pub fn run() {}" > src-tauri/src/lib.rs \
 && echo "fn main() {}" > src-tauri/src/main.rs
RUN cargo build --release -p donna-server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=build /app/target/release/donna-server /usr/local/bin/donna-server
VOLUME /data
ENV DONNA_DATA_DIR=/data
EXPOSE 8377
CMD ["donna-server"]
