FROM rust:1.73 as build

# create a new empty shell project
RUN USER=root cargo new --bin wannist
WORKDIR /wannist

# copy over your manifests
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

# this build step will cache your dependencies
RUN cargo build --release
RUN rm src/*.rs

# copy your source tree
COPY ./src ./src

# build for release
RUN rm ./target/release/deps/wannist*
RUN cargo build --release

# our final base
FROM gcr.io/distroless/cc AS runtime

# copy the build artifact from the build stage
COPY --from=build /wannist/target/release/wannist .

# set the startup command to run your binary
ENTRYPOINT ["/wannist"]
