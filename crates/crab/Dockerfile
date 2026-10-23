# `crab dev`, the local host. Built from the repository root by
# `docker-compose.yml` there.

FROM rust:1-bookworm AS crab
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p crab-agent \
    && cp target/release/crab /usr/local/bin/crab

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=crab /usr/local/bin/crab /usr/local/bin/crab
VOLUME /data
EXPOSE 7777
# Listens on every interface in the container; the service carries no
# authorization. Publish the port on loopback only.
ENTRYPOINT ["crab", "dev", "--addr", "0.0.0.0:7777", "--root", "/data"]
