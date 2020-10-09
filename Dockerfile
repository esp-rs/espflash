FROM espressif/idf:release-v4.1

ARG toolchain=nightly-2020-10-09
ARG xtensalink="https://github.com/espressif/crosstool-NG/releases/download/esp-2020r3/xtensa-esp32-elf-gcc8_4_0-esp-2020r3-linux-amd64.tar.gz"

RUN apt-get update -y && apt-get install -y build-essential cmake clang-6.0

# Init rust-xtensa
RUN git clone --depth 1 https://github.com/MabezDev/rust-xtensa /rust-xtensa
RUN cd rust-xtensa && \
  ./configure --experimental-targets=Xtensa && \
  ./x.py build --stage 2

ENV CUSTOM_RUSTC=/rust-xtensa
ENV RUST_BACKTRACE=1
ENV XARGO_RUST_SRC=$CUSTOM_RUSTC/library
ENV RUSTC=$CUSTOM_RUSTC/build/x86_64-unknown-linux-gnu/stage2/bin/rustc
ENV RUSTDOC=$CUSTOM_RUSTC/build/x86_64-unknown-linux-gnu/stage2/bin/rustdoc

# Init xtensa-esp32-elf-gcc8
RUN wget ${xtensalink} -O /tmp/xtensa-esp32-elf-gcc8.tar.gz && \
  mkdir -p /esp && \
  tar -xzf "/tmp/xtensa-esp32-elf-gcc8.tar.gz" -C /esp && \
  rm "/tmp/xtensa-esp32-elf-gcc8.tar.gz"
ENV PATH="$PATH:/esp/xtensa-esp32-elf/bin"

# Init espflash
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- --default-toolchain none -y
ENV PATH="/root/.cargo/bin:${PATH}"
RUN rustup toolchain install ${toolchain} --allow-downgrade --profile minimal
RUN cargo install cargo-espflash

WORKDIR /espflash

ENTRYPOINT ["cargo", "espflash"]
