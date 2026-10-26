# Builds and runs the DESIGN:OS Studio MCP server (designos-mcp) over stdio.
# Used by MCP directories such as Glama to introspect the tool list.
FROM rust:1-bookworm AS build
RUN apt-get update && apt-get install -y --no-install-recommends \
      pkg-config libxkbcommon-dev libwayland-dev libx11-dev libxcursor-dev \
      libxrandr-dev libxi-dev libgl1-mesa-dev libfontconfig1-dev \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY . .
RUN cargo build --release -p designos-mcp --bin designos-mcp

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
      libfontconfig1 libxkbcommon0 libgl1 ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/designos-mcp /usr/local/bin/designos-mcp
ENTRYPOINT ["designos-mcp"]
