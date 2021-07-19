FROM registry.gitlab.com/rust_musl_docker/image:nightly-2021-07-05

WORKDIR /workdir/

# build diesel so we can run migrations from this too
RUN cargo install diesel_cli --target=x86_64-unknown-linux-musl --no-default-features --features "postgres"

# install cargo web. don't need it as musl because we're going to use it from
# the first image to compile da wasm
RUN cargo install cargo-web

# Avoid having to install/build all dependencies by copying
# the Cargo files and making a dummy src/main.rs
COPY Cargo.toml .
COPY Cargo.lock .
RUN mkdir -p src
RUN mkdir -p editor/src
COPY editor/Cargo.toml ./editor/

RUN echo "fn main() {}" > src/main.rs
RUN echo "fn main() {}" > editor/src/main.rs
RUN cargo build --target=x86_64-unknown-linux-musl --release
RUN cargo web build --target=wasm32-unknown-unknown --package editor --release

# We need to touch our real main.rs file or else docker will use
# the cached one.
COPY . .
RUN touch src/main.rs

# we don't need to run tests.... this just causes a debug build
#RUN cargo test

# build the server binary (irc bot + webserver)
RUN cargo build --release -vv --target=x86_64-unknown-linux-musl --bin irctest
RUN cargo build --release -vv --target=x86_64-unknown-linux-musl --bin gen_js_env
RUN cargo build --release -vv --target=x86_64-unknown-linux-musl --bin run_multi

# build da wasm editor
RUN cargo web build --release --target=wasm32-unknown-unknown --package editor
RUN cp /workdir/target/wasm32-unknown-unknown/release/editor.wasm /workdir/static/editor.wasm
RUN cp /workdir/target/wasm32-unknown-unknown/release/editor.js /workdir/static/editor.js

# Start building the final image
FROM scratch
WORKDIR /
COPY --from=0 /root/.cargo/bin/diesel .
COPY --from=0 /workdir/target/x86_64-unknown-linux-musl/release/irctest .
COPY --from=0 /workdir/target/x86_64-unknown-linux-musl/release/gen_js_env .
COPY --from=0 /workdir/target/x86_64-unknown-linux-musl/release/run_multi .
COPY --from=0 /workdir/migrations ./migrations
COPY --from=0 /workdir/static ./static
COPY --from=0 /etc/ssl /etc/ssl

ENTRYPOINT ["./run_multi", "./gen_js_env ./static/env.js", "./diesel migration run", "./irctest"]
