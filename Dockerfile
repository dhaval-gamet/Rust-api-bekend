# ✅ Use latest compatible Rust version (1.83)
FROM rust:1.83 as builder

WORKDIR /app
COPY . .
RUN cargo build --release

# Now use a smaller image to run
FROM debian:buster-slim
WORKDIR /app
COPY --from=builder /app/target/release/excel_ai_api /app/excel_ai_api

CMD ["./excel_ai_api"]