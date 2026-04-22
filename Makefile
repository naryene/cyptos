# CyptOS - RISC-V Security-Focused MicrokernelPDP-11
# Makefile for Docker-based builds

.PHONY: all build release run debug test fmt clippy doc clean docker-build docker-shell audit

# Configuration
DOCKER_IMAGE := cyptos-builder
DOCKER_TAG := latest
KERNEL_ELF := target/riscv64gc-unknown-none-elf/release/kernel
KERNEL_BIN := target/riscv64gc-unknown-none-elf/release/kernel.bin

BUILD_STD := -Z build-std=core,alloc,compiler_builtins \
             -Z build-std-features=compiler-builtins-mem

# QEMU Configuration
QEMU := qemu-system-riscv64
QEMU_ARGS := -machine virt \
             -nographic \
             -bios none \
             -smp 4 \
             -m 128M \
             -kernel $(KERNEL_BIN)

QEMU_DEBUG_ARGS := $(QEMU_ARGS) -s -S

# Docker run command (no TTY for non-interactive builds)
DOCKER_RUN := docker run --rm \
              -v $(CURDIR):/workspace \
              -w /workspace \
              $(DOCKER_IMAGE):$(DOCKER_TAG)

# Docker run with TTY for interactive sessions
DOCKER_RUN_IT := docker run --rm -it \
              -v $(CURDIR):/workspace \
              -w /workspace \
              $(DOCKER_IMAGE):$(DOCKER_TAG)

# ============================================================================
# Build Targets
# ============================================================================

all: build

# Build in debug mode
build:
	$(DOCKER_RUN) cargo build $(BUILD_STD)

# Build in release mode
release:
	$(DOCKER_RUN) cargo build --release $(BUILD_STD)
	$(DOCKER_RUN) rust-objcopy --binary-architecture=riscv64 \
		$(KERNEL_ELF) --strip-all -O binary $(KERNEL_BIN)

# ============================================================================
# Run Targets
# ============================================================================

# Run the kernel in QEMU
run: release
	$(DOCKER_RUN) $(QEMU) $(QEMU_ARGS)

# Run with GDB server enabled (connect with gdb-multiarch)
debug: release
	@echo "Starting QEMU with GDB server on port 1234..."
	@echo "Connect with: gdb-multiarch -ex 'target remote :1234' $(KERNEL_ELF)"
	$(DOCKER_RUN) $(QEMU) $(QEMU_DEBUG_ARGS)

# ============================================================================
# Quality Targets
# ============================================================================

# Run tests (unit tests for crates that support it)
test:
	$(DOCKER_RUN) cargo test --target x86_64-unknown-linux-gnu -p cyptos-test

# Format code
fmt:
	$(DOCKER_RUN) cargo fmt --all

# Check formatting without modifying
fmt-check:
	$(DOCKER_RUN) cargo fmt --all -- --check

check:
	$(DOCKER_RUN) cargo check $(BUILD_STD)

# Run clippy lints
clippy:
	$(DOCKER_RUN) cargo clippy -p kernel $(BUILD_STD) -- -D warnings
	$(DOCKER_RUN) cargo clippy -p cyptos-test -p cyptos-test-macros \
		--target x86_64-unknown-linux-gnu --all-targets -- -D warnings

# Generate documentation
doc:
	$(DOCKER_RUN) cargo doc --no-deps --document-private-items

# Security audit (check for unsafe patterns)
audit:
	$(DOCKER_RUN) cargo clippy -- \
		-D clippy::undocumented_unsafe_blocks \
		-D clippy::multiple_unsafe_ops_per_block

# ============================================================================
# Docker Targets
# ============================================================================

# Build the Docker development image
docker-build:
	docker build -t $(DOCKER_IMAGE):$(DOCKER_TAG) -f tools/docker/Dockerfile .

# Open a shell in the Docker container
docker-shell:
	$(DOCKER_RUN_IT) /bin/bash

# ============================================================================
# Utility Targets
# ============================================================================

# Clean build artifacts
clean:
	$(DOCKER_RUN) cargo clean
	rm -f $(KERNEL_BIN)

# Show disassembly of the kernel
objdump: release
	$(DOCKER_RUN) rust-objdump -d $(KERNEL_ELF) | less

# Show kernel size
size: release
	$(DOCKER_RUN) rust-size $(KERNEL_ELF)

# ============================================================================
# Help
# ============================================================================

help:
	@echo "CyptOS Build System"
	@echo ""
	@echo "Build targets:"
	@echo "  build        - Build in debug mode"
	@echo "  release      - Build in release mode"
	@echo "  clean        - Remove build artifacts"
	@echo ""
	@echo "Run targets:"
	@echo "  run          - Run kernel in QEMU"
	@echo "  debug        - Run with GDB server (port 1234)"
	@echo ""
	@echo "Quality targets:"
	@echo "  test         - Run unit tests"
	@echo "  fmt          - Format code"
	@echo "  clippy       - Run lints"
	@echo "  doc          - Generate documentation"
	@echo "  audit        - Security audit for unsafe code"
	@echo ""
	@echo "Docker targets:"
	@echo "  docker-build - Build development container"
	@echo "  docker-shell - Open shell in container"
