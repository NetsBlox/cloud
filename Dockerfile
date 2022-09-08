FROM rust:1.61.0 as builder
WORKDIR /usr/src/netsblox
COPY . .
RUN cargo install --path crates/cloud --locked

FROM debian:bullseye-slim
RUN apt-get update && apt-get install -y libssl1.1 ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/netsblox-cloud /usr/local/bin/netsblox-cloud
CMD ["netsblox-cloud"]
