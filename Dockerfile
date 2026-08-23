FROM rust:1.97 AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release
COPY src ./src
RUN touch src/main.rs src/utils.rs && cargo build --release

FROM debian:bookworm-slim
WORKDIR /app
COPY --from=builder /app/target/release/fibonacci-cli /usr/local/bin/
ENV ADDR=0.0.0.0:3000
EXPOSE 3000
CMD ["fibonacci-cli"]
