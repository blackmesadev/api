# Build stage
FROM rust:1.75 as builder

WORKDIR /app
COPY . .

# Build the API
RUN cargo build --release -p mesa-api

# Runtime stage
FROM debian:bookworm-slim

# Install required packages
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN useradd -r -s /bin/false mesa

WORKDIR /app

# Copy binary from builder stage
COPY --from=builder /app/target/release/mesa-api /app/mesa-api

# Change ownership to app user
RUN chown -R mesa:mesa /app
USER mesa

EXPOSE 8080

CMD ["./mesa-api"]
