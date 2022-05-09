FROM debian:bullseye-slim

ENV DEBIAN_FRONTEND=noninteractive
ENV LC_ALL=C.UTF-8
ENV LANG=C.UTF-8

WORKDIR /root

# Install dependencies and required tools, then clean up after ourselves.
RUN apt-get update \
 && apt-get install -y build-essential curl libudev-dev pkg-config xz-utils  \
 && apt-get clean -y \
 && rm -rf /var/lib/apt/lists/* /tmp/library-scripts

# Install the stable Rust toolchain along with all targets supported by the ESP32-C3.
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --profile minimal \
 && $HOME/.cargo/bin/rustup component add rust-src \
 && $HOME/.cargo/bin/rustup target add \
     riscv32i-unknown-none-elf \
     riscv32imc-unknown-none-elf \
     riscv32imac-unknown-none-elf

# Install the fork of the Rust toolchain with Xtensa support, along with any other required build
# tools. Additionally installs `cargo espflash` and `espflash` while we're at it.
RUN curl -LO https://raw.githubusercontent.com/esp-rs/rust-build/main/install-rust-toolchain.sh \
 && chmod +x install-rust-toolchain.sh \
 && ./install-rust-toolchain.sh \
     --extra-crates "cargo-espflash espflash" \
     --clear-cache "YES" \
     --export-file export-rust.sh \
 && cat export-rust.sh >> $HOME/.bashrc \
 && rm -rf install-rust-toolchain.sh export-rust.sh /tmp/cargo-install*
