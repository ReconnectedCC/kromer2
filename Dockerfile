FROM rust:slim-bookworm AS chef
RUN cargo install cargo-chef
WORKDIR /usr/src/kromer

FROM chef AS planner
WORKDIR /usr/src/kromer
COPY . .
RUN cargo chef prepare  --recipe-path recipe.json

FROM chef AS builder
WORKDIR /usr/src/kromer
COPY --from=planner /usr/src/kromer/recipe.json recipe.json
RUN apt-get update && apt-get install -y pkg-config openssl libssl-dev curl && rm -rf /var/lib/apt/lists/*
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release

# Astro build stage
#FROM node:20-slim AS astro-base
#RUN npm install -g pnpm 
#WORKDIR /usr/src/astro
#COPY docs/pnpm-lock.yaml docs/package.json ./
#RUN pnpm install --frozen-lockfile
#COPY docs .
#RUN pnpm run build

FROM debian:bookworm-slim AS runtime
WORKDIR /kromer
RUN apt-get update && apt-get install -y pkg-config openssl libssl-dev && rm -rf /var/lib/apt/lists/*
COPY migrations .
COPY --from=builder /usr/src/kromer/target/release/kromer /usr/local/bin
# COPY --from=astro-base /usr/src/astro/dist /kromer/docs/dist
CMD ["kromer"]
