FROM --platform=linux/arm64 rust:1.86.0-bookworm@sha256:300ec56abce8cc9448ddea2172747d048ed902a3090e6b57babb2bf19f754081 AS build
RUN apt-get update \
    && apt-get install -y --no-install-recommends clang=1:14.0-55.7~deb12u1 protobuf-compiler=3.21.12-3+deb12u1 \
    && rm -rf /var/lib/apt/lists/* \
    && rustup target add wasm32-unknown-unknown \
    && useradd --create-home --uid 1000 --shell /bin/bash satoshi
ENV CARGO_HOME=/home/satoshi/.cargo
USER satoshi
WORKDIR /home/satoshi
COPY --chown=satoshi:satoshi . /home/satoshi/repo
RUN sh /home/satoshi/repo/build.sh

FROM scratch
COPY --from=build /home/satoshi/out/ /
