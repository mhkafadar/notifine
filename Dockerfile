FROM rust:1.77.2 AS builder
COPY . .
RUN cargo build --release

FROM ubuntu:22.04
EXPOSE 8080

RUN apt update && \
    apt install build-essential pkg-config libssl-dev libpq-dev ca-certificates -y && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder ./target/release/notifine ./target/release/notifine
RUN chmod +x ./target/release/notifine
CMD ["/target/release/notifine"]
