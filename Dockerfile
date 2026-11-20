FROM rust:1.85-bookworm AS builder
WORKDIR /workspace
COPY . .
RUN cargo build --release -p lifeboat-vault-store

FROM debian:bookworm-slim
RUN useradd --system --uid 10001 --home-dir /var/lib/lifeboat --create-home lifeboat \
    && mkdir -p /var/lib/lifeboat/vault \
    && chown -R lifeboat:lifeboat /var/lib/lifeboat
COPY --from=builder /workspace/target/release/lifeboat-vault-store /usr/local/bin/lifeboat-vault
USER 10001:10001
ENV LIFEBOAT_BIND=0.0.0.0:8080
ENV LIFEBOAT_VAULT_ROOT=/var/lib/lifeboat/vault
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/lifeboat-vault"]
