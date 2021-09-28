FROM ubuntu:20.04 AS build

ARG DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y \
    apt-utils \
    ca-certificates \
    curl \
    flex \
    git \
    unzip \
    wget \
    xz-utils \
    zip \
    python-is-python3 \
    ninja-build \
    build-essential \
    cmake \
    clang-6.0 \
   && apt-get autoremove -y \
   && rm -rf /var/lib/apt/lists/*

# (In)validate docker cache with latest commit information
ADD https://api.github.com/repos/esp-rs/espflash/commits/master /dev/null
# Init rust-xtensa
RUN git clone https://github.com/esp-rs/rust /rust-xtensa
RUN cd rust-xtensa && git submodule init && git submodule update
RUN cd rust-xtensa && \
  ./configure --experimental-targets=Xtensa && \
  ./x.py build --stage 2

RUN rm -r /rust-xtensa/src/llvm-project

FROM ubuntu:20.04

ARG DEBIAN_FRONTEND=noninteractive
ARG toolchain=nightly-2021-08-03
ARG xtensa_esp32_link="https://github.com/espressif/crosstool-NG/releases/download/esp-2021r1/xtensa-esp32-elf-gcc8_4_0-esp-2021r1-linux-amd64.tar.gz"
ARG xtensa_lx106_link="https://dl.espressif.com/dl/xtensa-lx106-elf-linux64-1.22.0-100-ge567ec7-5.2.0.tar.gz"

RUN apt-get update && apt-get install -y \
    apt-utils \
    ca-certificates \
    curl \
    git \
    unzip \
    wget \
    zip \
    python-is-python3 \
    build-essential \
   && apt-get autoremove -y \
   && rm -rf /var/lib/apt/lists/*

COPY --from=build /rust-xtensa/library /rust-xtensa/library
COPY --from=build /rust-xtensa/build/x86_64-unknown-linux-gnu/stage2 /rust-xtensa/build/x86_64-unknown-linux-gnu/stage2
COPY --from=build /rust-xtensa/src /rust-xtensa/build/x86_64-unknown-linux-gnu/stage2/lib/rustlib/src/rust/src
COPY --from=build /rust-xtensa/Cargo.toml /rust-xtensa/build/x86_64-unknown-linux-gnu/stage2/lib/rustlib/src/rust/
COPY --from=build /rust-xtensa/Cargo.lock /rust-xtensa/build/x86_64-unknown-linux-gnu/stage2/lib/rustlib/src/rust/

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- --default-toolchain none -y
ENV PATH="/root/.cargo/bin:${PATH}"
RUN rustup toolchain install ${toolchain} --allow-downgrade --profile minimal

ENV CUSTOM_RUSTC=/rust-xtensa
ENV XARGO_RUST_SRC=$CUSTOM_RUSTC/library
ENV RUSTC=$CUSTOM_RUSTC/build/x86_64-unknown-linux-gnu/stage2/bin/rustc
ENV RUSTDOC=$CUSTOM_RUSTC/build/x86_64-unknown-linux-gnu/stage2/bin/rustdoc

# Init xtensa-esp32-elf-gcc8
RUN wget -q ${xtensa_esp32_link} -O /tmp/xtensa-esp32-elf-gcc8.tar.gz && \
  mkdir -p /esp && \
  tar -xzf "/tmp/xtensa-esp32-elf-gcc8.tar.gz" -C /esp && \
  rm "/tmp/xtensa-esp32-elf-gcc8.tar.gz"
ENV PATH="$PATH:/esp/xtensa-esp32-elf/bin"
RUN wget -q ${xtensa_lx106_link} -O /tmp/xtensa-lx106-elf.tar.gz && \
  mkdir -p /esp && \
  tar -xzf "/tmp/xtensa-lx106-elf.tar.gz" -C /esp && \
  rm "/tmp/xtensa-lx106-elf.tar.gz"
ENV PATH="$PATH:/esp/xtensa-lx106-elf/bin"

# Init espflash
RUN cargo install cargo-espflash cargo-xbuild xargo

WORKDIR /espflash

ENTRYPOINT ["cargo", "espflash"]
