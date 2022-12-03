FROM rust:1.65.0 as builder
WORKDIR /zkill-history-importer/source
COPY /Cargo.toml .
COPY /Cargo.lock .
COPY /src ./src
RUN cargo install --locked --path .

FROM rust:1.65.0
WORKDIR /zkill-ws-importer
COPY --from=builder /zkill-history-importer/source/target/release/zkill-history-importer /zkill-history-importer
COPY /prod/prod_config.json config.json

ENTRYPOINT  ["./zkill-history-importer", "config.json"]
