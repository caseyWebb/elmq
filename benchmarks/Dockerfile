FROM --platform=linux/amd64 rust:1-slim AS builder

RUN apt-get update && apt-get install -y git && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY src/ src/
RUN cargo build --release --locked

FROM --platform=linux/amd64 node:22-slim

RUN apt-get update && apt-get install -y git jq && rm -rf /var/lib/apt/lists/*

RUN npm install -g elm@0.19.1-6 @anthropic-ai/claude-code

COPY --from=builder /build/target/release/elmq /usr/local/bin/elmq

COPY benchmarks/fixture/ /bench/fixture/
COPY benchmarks/scenarios/ /bench/scenarios/
COPY benchmarks/system-prompt.md /bench/system-prompt.md
COPY benchmarks/mcp-config.json /bench/mcp-config.json
COPY benchmarks/run.sh /bench/run.sh
COPY benchmarks/analyze.sh /bench/analyze.sh
RUN chmod +x /bench/run.sh /bench/analyze.sh

# Pre-populate elm-stuff so runs don't need network
WORKDIR /bench/fixture
RUN elm make src/Main.elm --output=/dev/null 2>&1 || true

WORKDIR /bench
