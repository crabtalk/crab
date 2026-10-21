# bermd with its dashboard, for development. Built from the repository root by
# `docker-compose.yml` there.

FROM oven/bun:1 AS dashboard
WORKDIR /src
COPY www/src/app.css www/src/app.css
COPY dashboard/package.json dashboard/bun.lock dashboard/
RUN cd dashboard && bun install --frozen-lockfile
COPY dashboard dashboard
RUN cd dashboard && bun run build

FROM rust:1-bookworm AS bermd
WORKDIR /src
COPY . .
COPY --from=dashboard /src/dashboard/build dashboard/build
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p bermd --features dashboard \
    && cp target/release/bermd /usr/local/bin/bermd

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=bermd /usr/local/bin/bermd /usr/local/bin/bermd
VOLUME /data
EXPOSE 7777
# Every interface, inside the container: publish the port on loopback only,
# since the service carries no authorization.
ENTRYPOINT ["bermd", "--addr", "0.0.0.0:7777", "--root", "/data"]
