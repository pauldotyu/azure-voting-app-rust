FROM rust:1.65.0 as build

WORKDIR /usr/src/app
COPY . /usr/src/app

RUN cargo build

ENV PORT 8080
EXPOSE 8080

CMD ["cargo", "run", "-q"]
