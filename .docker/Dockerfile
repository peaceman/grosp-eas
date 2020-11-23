FROM rust:1.46 as compile-image

RUN USER=root rustup component add rustfmt
RUN USER=root cargo new --bin /usr/src/edge-auto-scaler
WORKDIR /usr/src/edge-auto-scaler

COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

RUN cargo build --release
RUN rm src/*.rs

COPY ./src ./src
COPY ./proto ./proto
COPY ./build.rs ./build.rs
RUN cargo build --release

FROM debian:buster-slim as runtime-image

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y dumb-init ca-certificates tzdata libssl-dev \
    && rm -rf /var/lib/apt/lists*

ENV APP_USER=appuser \
    APP_DIR=/opt/app

RUN groupadd $APP_USER \
    && useradd -g $APP_USER $APP_USER \
    && mkdir -p $APP_DIR

COPY --from=compile-image /usr/src/edge-auto-scaler/target/release/edge-auto-scaler $APP_DIR/edge-auto-scaler

RUN chown -R $APP_USER:$APP_USER $APP_DIR

USER $APP_USER
WORKDIR $APP_DIR

ENTRYPOINT ["dumb-init", "--"]

CMD ["./edge-auto-scaler"]
