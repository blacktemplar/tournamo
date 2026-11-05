# The dev container: the toolchains of the hooks (see AGENTS.md), Chromium for the browser tests,
# Node for Zed and its external agents, and Claude Code. Everything is installed as root and owned by
# root, so the user `dev` working in the container cannot change it; there is no sudo.
FROM node:24-trixie-slim AS node
FROM ghcr.io/astral-sh/uv:0.13 AS uv

FROM debian:trixie-slim

ARG RUST_VERSION=stable

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential \
        ca-certificates \
        chromium \
        chromium-driver \
        curl \
        gh \
        git \
        less \
        pkg-config \
        procps \
        python3 \
        ripgrep \
    && rm -rf /var/lib/apt/lists/*

# without the user namespaces the container may not create, Chromium cannot start its sandbox; it
# only opens the pages built here, which are served locally, so it ignores the proxy and stays
# offline (rather than trying Google's services in the background)
RUN echo 'export CHROMIUM_FLAGS="$CHROMIUM_FLAGS --no-sandbox --no-proxy-server"' \
    > /etc/chromium.d/no-sandbox

COPY --from=node /usr/local/bin/node /usr/local/bin/node
COPY --from=node /usr/local/lib/node_modules /usr/local/lib/node_modules
RUN ln -s ../lib/node_modules/npm/bin/npm-cli.js /usr/local/bin/npm \
    && ln -s ../lib/node_modules/npm/bin/npx-cli.js /usr/local/bin/npx

# for prek, which installs the Python hooks with it
COPY --from=uv /uv /uvx /usr/local/bin/

# rustup and its toolchains stay read-only; CARGO_HOME (registry, git checkouts) is set to the
# user's own below, so cargo finds the binaries through PATH
ENV RUSTUP_HOME=/usr/local/rustup \
    PATH=/usr/local/cargo/bin:$PATH
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | CARGO_HOME=/usr/local/cargo sh -s -- -y --no-modify-path --profile minimal \
            --default-toolchain "$RUST_VERSION" \
            --component clippy,rustfmt,rust-analyzer,rust-src \
            --target wasm32-unknown-unknown \
    && CARGO_HOME=/usr/local/cargo rustup toolchain install nightly --profile minimal --component rustfmt \
    && chmod -R a+rX /usr/local/rustup /usr/local/cargo

# the tools of the hooks and cargo-warloc (counts lines of code), prebuilt; dioxus-cli of the
# version of dioxus in Cargo.lock
ARG DIOXUS_CLI_VERSION=0.7.10
RUN curl -L --proto '=https' --tlsv1.2 -sSf \
        https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh \
        | CARGO_HOME=/usr/local/cargo bash \
    && CARGO_HOME=/usr/local/cargo cargo binstall -y --locked \
        prek wasm-pack trunk "dioxus-cli@$DIOXUS_CLI_VERSION" cargo-warloc \
    && rm -rf /usr/local/cargo/registry /usr/local/cargo/git

# Claude Code for the terminal; Zed's Claude Agent brings its own
RUN npm install -g @anthropic-ai/claude-code && npm cache clean --force
COPY managed-settings.json /etc/claude-code/managed-settings.json

# gh with the read-only token from the host (README.md, "GitHub CLI")
COPY --chmod=755 gh /usr/local/bin/gh

# remotes keep their SSH URLs for the host (.git/config is shared and read-only here), but only HTTPS
# gets through the proxy; without credentials this fetches and cannot push
RUN git config --system url.https://github.com/.insteadOf git@github.com: \
    && git config --system --add url.https://github.com/.insteadOf ssh://git@github.com/

RUN useradd --create-home --uid 1000 --shell /bin/bash dev \
    && mkdir -p /home/dev/.cargo /home/dev/.claude /home/dev/.cache \
    && chown dev:dev /home/dev/.cargo /home/dev/.claude /home/dev/.cache

# dx's telemetry off: what `dx config set disable-telemetry true` writes, which runs only in a cargo
# project and has no environment variable
RUN mkdir -p /home/dev/.local/share/.dx \
    && echo 'disable_telemetry = true' > /home/dev/.local/share/.dx/settings.toml \
    && chown -R dev:dev /home/dev/.local

ENV CARGO_HOME=/home/dev/.cargo \
    CHROMEDRIVER=/usr/bin/chromedriver \
    E2E_CHROMEDRIVER=/usr/bin/chromedriver

USER dev
WORKDIR /home/dev
