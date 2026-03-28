FROM node:22-slim

RUN apt-get update && apt-get install -y git jq && rm -rf /var/lib/apt/lists/*

RUN npm install -g elm@0.19.1-6 @anthropic-ai/claude-code

COPY elmq /usr/local/bin/elmq

COPY fixture/ /bench/fixture/
COPY scenarios/ /bench/scenarios/
COPY system-prompt.md /bench/system-prompt.md
COPY mcp-config.json /bench/mcp-config.json
COPY run.sh /bench/run.sh
COPY analyze.sh /bench/analyze.sh
RUN chmod +x /bench/run.sh /bench/analyze.sh

# Pre-populate elm-stuff so runs don't need network
WORKDIR /bench/fixture
RUN elm make src/Main.elm --output=/dev/null 2>&1 || true

WORKDIR /bench
