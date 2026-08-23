# build stage
FROM rust:1.85-slim AS build
WORKDIR /src
COPY . .
RUN cargo build --release -p markv

# runtime: binary needs only glibc, which the base image ships
FROM debian:bookworm-slim
COPY --from=build /src/target/release/markv /usr/local/bin/markv
EXPOSE 9379
VOLUME ["/data"]
ENTRYPOINT ["markv"]
CMD ["/data"]
