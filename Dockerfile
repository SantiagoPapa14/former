FROM rust:1.87-slim

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .
RUN cargo build --release

EXPOSE 3000
CMD ["./target/release/former"]
