FROM ubuntu:22.04 AS build

RUN apt-get update && apt-get install -y \
        curl build-essential ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --default-toolchain none --profile minimal
ENV PATH="/root/.cargo/bin:${PATH}"

COPY . /build/
WORKDIR /build
RUN cargo build --release

FROM ubuntu:22.04

RUN apt-get update && apt-get install -y socat \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -m -s /bin/bash ctf

COPY --from=build /build/target/release/nanolog /challenge/nanolog
RUN chmod 755 /challenge/nanolog

WORKDIR /challenge
EXPOSE 1337

CMD ["socat", \
     "TCP-LISTEN:1337,reuseaddr,fork,nodelay", \
     "EXEC:/challenge/nanolog,nofork"]
