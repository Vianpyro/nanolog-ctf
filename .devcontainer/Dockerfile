FROM debian:bookworm-slim

# Install build dependencies and common tools
RUN apt-get update && apt-get install -y --no-install-recommends \
    bash \
    git \
    gh \
    curl \
    ca-certificates \
    python3 \
    gcc \
    libc6-dev \
    binutils \
    file \
    xxd \
    less \
    && rm -rf /var/lib/apt/lists/*

# Setup default user
ARG USERNAME=vscode
ARG USER_UID=1000
ARG USER_GID=$USER_UID
RUN groupadd --gid $USER_GID $USERNAME && \
    useradd --uid $USER_UID --gid $USER_GID -m -s /bin/bash $USERNAME

# Switch to the default user
USER $USERNAME

# Rust installé dans le home de l'utilisateur pour éviter les problèmes de permissions
ENV RUSTUP_HOME=/home/${USERNAME}/.rustup \
    CARGO_HOME=/home/${USERNAME}/.cargo \
    PATH=/home/${USERNAME}/.cargo/bin:$PATH

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --default-toolchain stable --profile default \
    && rustup component add rust-src rustfmt

WORKDIR /workspace
