# =============================================================================
# Variables
# =============================================================================

# Project name - should match the name in Cargo.toml
BINARY_NAME := tt

# Build directories
TARGET_DIR := target
RELEASE_DIR := $(TARGET_DIR)/release

# Binary paths
RELEASE_BINARY := $(RELEASE_DIR)/$(BINARY_NAME)

# Installation directory (requires sudo)
INSTALL_DIR ?= /usr/local/bin

# Default arguments for 'make run'
ARGS ?= .

# Use shell to find cargo
CARGO := $(shell which cargo)

.DEFAULT_GOAL := help

# =============================================================================
# Main Targets
# =============================================================================

.PHONY: all
all: build ## Build the project in debug mode

.PHONY: build
build:
	@echo "Building $(BINARY_NAME) in debug mode..."
	@$(CARGO) build --locked

.PHONY: run
run:
	@echo "Running $(BINARY_NAME) with args: $(ARGS)"
	@$(CARGO) run -- $(ARGS)

.PHONY: release
release:
	@echo "Building $(BINARY_NAME) in release mode for native target..."
	@$(CARGO) build --release --locked

.PHONY: test
test: ## Run tests
	@echo "Running tests..."
	@$(CARGO) test --locked

.PHONY: clean
clean: ## Clean up build artifacts
	@echo "Cleaning up build artifacts..."
	@$(CARGO) clean

.PHONY: install
install: release ## Build in release mode and install the binary (requires sudo)
	@echo "Installing $(BINARY_NAME) to $(INSTALL_DIR)..."
	@sudo install -m 755 "$(RELEASE_BINARY)" "$(INSTALL_DIR)/"
	@echo "$(BINARY_NAME) installed successfully."

.PHONY: uninstall
uninstall: ## Uninstall the binary (requires sudo)
	@echo "Uninstalling $(BINARY_NAME) from $(INSTALL_DIR)..."
	@sudo rm -f "$(INSTALL_DIR)/$(BINARY_NAME)"
	@echo "$(BINARY_NAME) uninstalled."

.PHONY: help
help: ## Show this help message
	@echo "Usage: make [target]"
	@echo ""
	@echo "Available targets:"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-15s\033[0m %s\n", $$1, $$2}'
