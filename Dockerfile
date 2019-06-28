FROM registry.gitlab.com/rust_musl_docker/image:nightly-2019-06-27

WORKDIR /workdir/

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

# We need to touch our real main.rs file or else docker will use
# the cached one.
COPY . .
RUN touch src/main.rs

# we don't need to run tests.... this just causes a debug build
#RUN cargo test

RUN cargo build --release -vv --target=x86_64-unknown-linux-musl --bin irctest

# Start building the final image
FROM scratch
WORKDIR /home/rust/
COPY --from=0 /workdir/target/x86_64-unknown-linux-musl/release/irctest .
ENTRYPOINT ["./irctest"]
