FROM clux/muslrust:stable AS chef
WORKDIR /app
RUN cargo install cargo-chef
RUN apt update && apt install lld clang -y

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --target x86_64-unknown-linux-musl --recipe-path recipe.json
COPY . .
ENV SQLX_OFFLINE true
RUN cargo build --release --target x86_64-unknown-linux-musl --bin rust_email_newsletter

FROM alpine AS runtime
WORKDIR /app
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/rust_email_newsletter rust_email_newsletter
COPY configuration configuration
ENV APP_ENVIRONMENT production
ENTRYPOINT [ "./rust_email_newsletter" ]