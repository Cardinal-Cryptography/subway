FROM rust:1.74-slim-buster as builder
WORKDIR /app

ADD . .

RUN cargo build --release --verbose

# =============

FROM ubuntu:22.04

RUN useradd -m -u 1000 -U -s /bin/sh -d /app docker

WORKDIR /app

COPY --from=builder /app/target/release/subway /usr/local/bin
COPY ./configs/config.yml /app/config.yml

USER docker

ENTRYPOINT ["/usr/local/bin/subway", "--config=config.yml"]
