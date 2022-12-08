FROM rust:1.65 AS builder
COPY . .
RUN cargo build --release

FROM debian:bullseye-slim
EXPOSE 8080
RUN apt-get update && apt-get install postgresql -y
COPY --from=builder ./target/release/telegram-gitlab ./target/release/telegram-gitlab
RUN chmod +x ./target/release/telegram-gitlab
CMD ["/target/release/telegram-gitlab"]
