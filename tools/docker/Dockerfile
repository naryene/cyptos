# RISC-V OS Development Environment
# Self-contained toolchain - no host dependencies required

FROM ubuntu:24.04

# Avoid interactive prompts
ENV DEBIAN_FRONTEND=noninteractive
ENV TZ=UTC

# Install base dependencies
RUN apt-get update && apt-get install -y \
    curl \
    git \
    build-essential \
    qemu-system-riscv64 \
    gdb-multiarch \
    && rm -rf /var/lib/apt/lists/*

# Install Rust via rustup
ENV RUSTUP_HOME=/opt/rustup
ENV CARGO_HOME=/opt/cargo
ENV PATH="/opt/cargo/bin:${PATH}"

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain none

# Install the specific toolchain (will be determined by rust-toolchain.toml)
# Pre-install nightly with RISC-V target for faster builds
RUN rustup install nightly-2026-02-14 && \
    rustup default nightly-2026-02-14 && \
    rustup target add riscv64gc-unknown-none-elf && \
    rustup component add rust-src llvm-tools-preview rustfmt clippy

# Install cargo-binutils for objcopy, objdump, etc.
RUN cargo install cargo-binutils && \
    rustup component add llvm-tools-preview

# Create workspace directory
WORKDIR /workspace

# Default command
CMD ["/bin/bash"]
