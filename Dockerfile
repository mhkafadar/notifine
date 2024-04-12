FROM rust:1.77.2 AS builder
COPY . .
RUN cargo build --release

FROM debian:bullseye-slim
EXPOSE 8080
#RUN #apt-get update && apt-get install postgresql -y
RUN apt-get update && \
    apt-get install -y libpq5 libssl3 ca-certificates && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder ./target/release/notifine ./target/release/notifine
RUN chmod +x ./target/release/notifine
CMD ["/target/release/notifine"]
