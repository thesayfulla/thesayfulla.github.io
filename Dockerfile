# Build stage: compile the site into a single binary.
FROM rust:1.98-slim-bookworm AS builder
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY templates ./templates
RUN cargo build --release --locked


# Runtime stage: the binary plus its templates and static files.
FROM debian:bookworm-slim AS runtime

RUN useradd app

WORKDIR /app
COPY --from=builder /src/target/release/portfolio /usr/local/bin/portfolio
COPY --chown=app:app templates ./templates
COPY --chown=app:app static ./static

# The SQLite database and uploads live in /app; the "app" user writes them.
RUN chown app:app /app
USER app

ENV HOST=0.0.0.0 \
    PORT=8000
EXPOSE 8000

# Creates the database tables on first start. Make a user with:
#   docker exec -it <container> portfolio create-user <name>
CMD ["portfolio", "serve"]
