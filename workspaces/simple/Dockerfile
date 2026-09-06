FROM ubuntu:24.04

# Set shell to be bash and fail any pipe if any step in the pipe fails
SHELL ["/bin/bash", "-o", "pipefail", "-c"]
# Noninteractive apt so texlive/tzdata don't block the build on interactive prompts
ENV DEBIAN_FRONTEND=noninteractive

# install system packages
RUN apt-get update \
    # Installation requirements
    && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    gnupg2 \
    # node
    && curl -sL https://deb.nodesource.com/setup_lts.x | bash - \
    && apt-get update \
    && apt-get install -y --no-install-recommends \
    # Install system packages
    # jq is needed for the Nimbus platform
    jq \
    nano \
    vim \
    fish \
    zsh \
    build-essential \
    sudo \
    nodejs \
    git \
    htop \
    pandoc \
    texlive \
    texlive-science \
    texlive-latex-extra \
    texlive-luatex \
    texlive-xetex \
    texlive-pictures \
    latexmk \
    lmodern \
    # containers: podman only (docker-ce dropped) + podman-docker for a `docker` shim
    podman \
    podman-docker \
    fuse-overlayfs \
    uidmap \
    slirp4netns \
    less \
    libxext6 \
    libxrender1 \
    libxtst6 \
    libfreetype6 \
    libxi6 \
    openssl \
    unzip \
    # build toolchain for the occasional source wheel
    libpq-dev \
    libssl-dev \
    libffi-dev \
    zlib1g-dev \
    wget \
    xz-utils \
    && rm -rf /var/lib/apt/lists/*

# Install AWS CLI v2
RUN curl -fsSL "https://awscli.amazonaws.com/awscli-exe-linux-x86_64.zip" -o "awscliv2.zip" \
    && unzip -q awscliv2.zip \
    && ./aws/install \
    && rm -rf aws awscliv2.zip

# --- uv: Python toolchain manager (provides Python; replaces the python base + pyenv) ---
# uv downloads standalone CPython builds and manages venvs. Installed system-wide at build
# so the Jupyter infra venv (/opt/jupyter) can be built from a uv-managed Python; uv is
# also exposed to user shells via mise (see the mise block).
ENV UV_INSTALL_DIR=/usr/local/bin
# uv-managed Pythons go to a shared, world-readable dir (NOT root's ~/.local, which is
# mode 700) so the JupyterHub single-user server — which may spawn as a non-root user —
# can reach the interpreter that /opt/jupyter's bin/python symlinks to.
ENV UV_PYTHON_INSTALL_DIR=/opt/uv/python
RUN curl -LsSf https://astral.sh/uv/install.sh | sh

# --- mise: declarative tool manager (uv, bun, node, lefthook, gitleaks, sqruff) ---
# Tools are declared in /etc/mise/config.toml and pre-installed at build into a shared
# dir (/opt/mise) so they're present day one; mise is activated for interactive shells.
# Infra Node stays on NodeSource (non-interactive), independent of mise activation.
ENV MISE_DATA_DIR=/opt/mise
ENV MISE_GLOBAL_CONFIG_FILE=/etc/mise/config.toml
COPY mise/config.toml /etc/mise/config.toml
RUN export MISE_INSTALL_PATH=/usr/local/bin/mise \
    && curl -fsSL https://mise.run | sh \
    && mise install \
    && chmod -R a+rX /opt/mise \
    && for rc in /etc/bash.bashrc /etc/zsh/zshrc; do \
         msh="$([ "$rc" = /etc/zsh/zshrc ] && echo zsh || echo bash)"; \
         echo "command -v mise >/dev/null 2>&1 && eval \"\$(mise activate ${msh})\"" >> "$rc"; \
       done \
    && { [ -d /etc/fish ] && echo 'command -v mise >/dev/null 2>&1 && mise activate fish | source' >> /etc/fish/config.fish; } || true

# --- GitHub CLI (official repo) --------------------------------------------
RUN mkdir -p /etc/apt/keyrings \
    && curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg -o /etc/apt/keyrings/githubcli-archive-keyring.gpg \
    && chmod go+r /etc/apt/keyrings/githubcli-archive-keyring.gpg \
    && echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" \
       > /etc/apt/sources.list.d/github-cli.list \
    && apt-get update \
    && apt-get install -y --no-install-recommends gh \
    && rm -rf /var/lib/apt/lists/*

# Needed for JupyterHub
RUN npm install -g configurable-http-proxy \
    && rm -rf ~/.npm

# --- Python infrastructure venv (uv-managed Python) ------------------------
# The Jupyter/JupyterHub/code-server-proxy stack lives in an isolated venv built from a
# uv-managed CPython. There is no system Python and no pyenv — users manage their own
# environments with uv (available in every shell via mise). VIRTUAL_ENV + PATH make both
# `uv pip install` and the venv's own entrypoints (jupyterhub, jupyter) resolve here.
# Pinned inline (only three): JupyterHub stays on 4.x (5.3+ needs a jupyterhub_config.py
# this image's CMD doesn't supply); jupyter-server-proxy backs the code-server button.
RUN uv venv --python 3.11 /opt/jupyter \
    && chmod -R a+rX /opt/uv /opt/jupyter
ENV VIRTUAL_ENV=/opt/jupyter
ENV PATH="/opt/jupyter/bin:$PATH"
RUN uv pip install --no-cache \
    jupyterhub==4.0.2 \
    jupyterlab==3.6.5 \
    jupyter-server-proxy==4.0.0

# VS Code
COPY codeserver-proxy codeserver-proxy
RUN curl -fsSL https://code-server.dev/install.sh \
    | sh -s -- --version 4.16.1 \
    && uv pip install --no-cache ./codeserver-proxy

# VS Code CLI
RUN curl -Lk 'https://code.visualstudio.com/sha/download?build=stable&os=cli-alpine-x64' --output vscode_cli.tar.gz \
    && tar -xf vscode_cli.tar.gz \
    && cp ./code /usr/local/bin \
    && rm -rf vscode_cli.tar.gz

# Cursor CLI
RUN curl -Lk 'https://api2.cursor.sh/updates/download-latest?os=cli-alpine-x64' --output cursor_cli.tar.gz \
    && tar -xf cursor_cli.tar.gz \
    && cp ./cursor /usr/local/bin \
    && rm -rf cursor_cli.tar.gz

COPY vscode-tunnel vscode-tunnel

RUN mkdir /vscode-tunnel/logs \
    && mkdir /vscode-tunnel/docs \
    && chmod -R 777 vscode-tunnel

RUN cp vscode-tunnel/scripts/start.sh /usr/bin/start-tunnel \
    && cp vscode-tunnel/scripts/stop.sh /usr/bin/stop-tunnel \
    && cp vscode-tunnel/scripts/refresh.sh /usr/bin/restart-tunnel \
    && cp vscode-tunnel/scripts/status.sh /usr/bin/tunnel-status \
    && chmod +x /usr/bin/start-tunnel /usr/bin/stop-tunnel /usr/bin/restart-tunnel /usr/bin/tunnel-status

COPY TUNNELING.md /vscode-tunnel/docs/README.md

RUN git config --global credential.helper store \
    && git config --global core.filemode false

# Latest security/patch updates for the 24.04 base. Kept late so it doesn't invalidate the
# heavy base layers above.
RUN apt-get update && apt-get upgrade -y && rm -rf /var/lib/apt/lists/*

# Force shell to bash as the Nimbus platform isn't respecting JupyterHub option
RUN ln -sf /bin/bash /bin/sh

# Expose port and set the default command
EXPOSE 8000
CMD ["jupyterhub", \
    "--Spawner.default_url=/lab", \
    "--Spawner.http_timeout=90", \
    "--NotebookApp.terminado_settings={'shell_command':'[/bin/bash]'}" \
    ]
