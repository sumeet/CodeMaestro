ARG TOOLCHAIN=nightly-06-08
FROM ekidd/rust-musl-builder:${TOOLCHAIN} as builder

WORKDIR /home/rust/

# Avoid having to install/build all dependencies by copying
# the Cargo files and making a dummy src/main.rs
COPY Cargo.toml .
COPY Cargo.lock .
RUN mkdir -p editor/src
COPY editor/Cargo.toml ./editor/

RUN echo "fn main() {}" > src/main.rs
RUN echo "fn main() {}" > editor/src/main.rs
RUN cargo build --release

# We need to touch our real main.rs file or else docker will use
# the cached one.
COPY . .
RUN sudo touch src/main.rs

RUN cargo test
RUN cargo build --release --bin irctest

# Start building the final image
FROM scratch
WORKDIR /home/rust/
COPY --from=rust-musl-builder /home/rust/target/x86_64-unknown-linux-musl/release/irctest .
ENTRYPOINT ["./irctest"]
