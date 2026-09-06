FROM ubuntu:24.04

# Bash with pipefail so any step in a pipe fails the build; noninteractive apt.
SHELL ["/bin/bash", "-o", "pipefail", "-c"]
ENV DEBIAN_FRONTEND=noninteractive

# --- System packages -------------------------------------------------------
# Containers: podman only (docker-ce dropped) + podman-docker for a `docker` shim.
# Python: system python3.12 is present; the Jupyter infra runs in a venv (see below)
# and users manage their own envs with uv — so no pyenv and minimal build deps.
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
    ca-certificates curl gnupg lsb-release \
    && curl -sL https://deb.nodesource.com/setup_lts.x | bash - \
    && apt-get update \
    && apt-get install -y --no-install-recommends \
    # jq is needed for the Nimbus platform
    jq nano vim fish zsh sudo git htop pandoc less wget unzip xz-utils \
    # openssh-server: for Zed SSH remoting (option A) — see start-ssh
    openssh-server \
    # node
    nodejs \
    # python (system) — infra venv built from this, users use uv for their own
    python3 python3-pip python3-venv python3-dev \
    # containers
    podman podman-docker fuse-overlayfs uidmap slirp4netns \
    # build toolchain for the occasional source wheel
    build-essential libpq-dev libssl-dev libffi-dev zlib1g-dev \
    # LaTeX / document toolchain
    texlive texlive-science texlive-latex-extra texlive-luatex texlive-xetex texlive-pictures \
    latexmk lmodern \
    # X client libs
    libxext6 libxrender1 libxtst6 libfreetype6 libxi6 openssl \
    && rm -rf /var/lib/apt/lists/*

# --- AWS CLI v2 ------------------------------------------------------------
RUN curl -fsSL "https://awscli.amazonaws.com/awscli-exe-linux-x86_64.zip" -o "awscliv2.zip" \
    && unzip -q awscliv2.zip \
    && ./aws/install \
    && rm -rf aws awscliv2.zip

# --- dbt Fusion (uv, bun, node are managed by mise — see the mise block) ----
# dbt Fusion — Rust-based dbt engine (`dbt` CLI). SHELL exported so the installer's
# rc step doesn't abort the build (set -u).
RUN export SHELL=/bin/bash \
    && curl -fsSL https://public.cdn.getdbt.com/fs/install/install.sh \
       | sh -s -- --to /usr/local/bin \
    && ln -sf /usr/local/bin/dbt /usr/local/bin/dbtf

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

# --- Modern terminal/CLI tooling (apt) + GitHub CLI (official repo) --------
RUN mkdir -p /etc/apt/keyrings \
    && curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg -o /etc/apt/keyrings/githubcli-archive-keyring.gpg \
    && chmod go+r /etc/apt/keyrings/githubcli-archive-keyring.gpg \
    && echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" \
       > /etc/apt/sources.list.d/github-cli.list \
    && apt-get update \
    && apt-get install -y --no-install-recommends \
    ripgrep fd-find bat fzf tree tmux gh \
    && rm -rf /var/lib/apt/lists/* \
    # Ubuntu/Debian ship these under alternate names — expose the canonical commands
    && ln -sf /usr/bin/fdfind /usr/local/bin/fd \
    && ln -sf /usr/bin/batcat /usr/local/bin/bat

# Single-binary CLI tools not packaged in the distro (x86_64)
RUN curl -sS https://starship.rs/install.sh | sh -s -- -y -b /usr/local/bin \
    && curl -sSfL https://raw.githubusercontent.com/ajeetdsouza/zoxide/main/install.sh | sh -s -- --bin-dir /usr/local/bin \
    && curl -fsSL https://github.com/eza-community/eza/releases/latest/download/eza_x86_64-unknown-linux-gnu.tar.gz -o /tmp/eza.tgz \
    && tar -xzf /tmp/eza.tgz -C /usr/local/bin \
    && curl -fsSL https://github.com/mikefarah/yq/releases/latest/download/yq_linux_amd64 -o /usr/local/bin/yq \
    && chmod +x /usr/local/bin/yq \
    && DELTA_VER="$(curl -fsSL https://api.github.com/repos/dandavison/delta/releases/latest | jq -r .tag_name)" \
    && curl -fsSL "https://github.com/dandavison/delta/releases/download/${DELTA_VER}/git-delta_${DELTA_VER}_amd64.deb" -o /tmp/delta.deb \
    && dpkg -i /tmp/delta.deb \
    && rm -f /tmp/eza.tgz /tmp/delta.deb

# System-wide interactive shell DX: starship prompt, zoxide, fzf, eza aliases.
RUN for rc in /etc/bash.bashrc /etc/zsh/zshrc; do \
      sh="$([ "$rc" = /etc/zsh/zshrc ] && echo zsh || echo bash)"; \
      { \
        echo ''; \
        echo '# --- workspace DX defaults ---'; \
        echo "command -v starship >/dev/null 2>&1 && eval \"\$(starship init ${sh})\""; \
        echo "command -v zoxide   >/dev/null 2>&1 && eval \"\$(zoxide init ${sh})\""; \
        echo 'command -v eza >/dev/null 2>&1 && alias ls="eza --group-directories-first"'; \
        echo 'command -v eza >/dev/null 2>&1 && alias ll="eza -la --group-directories-first --git"'; \
        echo 'command -v eza >/dev/null 2>&1 && alias la="eza -a --group-directories-first"'; \
      } >> "$rc"; \
    done \
    && [ -f /usr/share/doc/fzf/examples/key-bindings.bash ] \
       && echo '[ -f /usr/share/doc/fzf/examples/key-bindings.bash ] && . /usr/share/doc/fzf/examples/key-bindings.bash' >> /etc/bash.bashrc || true

# Sensible tmux defaults (mouse scroll matters in browser/tunnel terminals)
RUN printf '%s\n' \
    'set -g mouse on' \
    'set -g history-limit 50000' \
    'set -g base-index 1' \
    'setw -g pane-base-index 1' \
    'set -g default-terminal "tmux-256color"' \
    > /etc/tmux.conf

# --- Desktop: XFCE (themed) + Firefox + KasmVNC ----------------------------
# DESKTOP-FIRST: KasmVNC is the workspace entrypoint, served at the ROOT path (Istio routes
# the workspace URL straight to it). At the root there's no URL prefix, so KasmVNC's
# websocket works (it hardcodes /websockify at the origin root — fine when KasmVNC *is* the
# root). KasmVNC is web-native and smoother than noVNC. Firefox is from Mozilla's apt repo
# (Ubuntu's `firefox` is a snap stub that fails in a container).
# NOTE: the running container needs `--security-opt seccomp=unconfined` for Xorg/dbus.
RUN install -d -m0755 /etc/apt/keyrings \
    && curl -fsSL https://packages.mozilla.org/apt/repo-signing-key.gpg -o /etc/apt/keyrings/packages.mozilla.org.asc \
    && echo "deb [signed-by=/etc/apt/keyrings/packages.mozilla.org.asc] https://packages.mozilla.org/apt mozilla main" \
       > /etc/apt/sources.list.d/mozilla.list \
    && printf 'Package: *\nPin: origin packages.mozilla.org\nPin-Priority: 1000\n' > /etc/apt/preferences.d/mozilla \
    && apt-get update \
    && apt-get install -y --no-install-recommends \
    dbus-x11 xfce4 xfce4-panel xfce4-session xfce4-settings xfce4-terminal xorg \
    firefox \
    arc-theme papirus-icon-theme fonts-firacode fonts-noto greybird-gtk-theme \
    # software Vulkan (lavapipe) so Zed can render in the GPU-less desktop
    mesa-vulkan-drivers libvulkan1 vulkan-tools \
    # KasmVNC (web-native VNC). Installed while apt lists are present so its deps
    # (ssl-cert, perl modules) resolve; clean the lists last.
    && KVER="$(curl -fsSL https://api.github.com/repos/kasmtech/KasmVNC/releases/latest | jq -r .tag_name)" \
    && curl -fsSL "https://github.com/kasmtech/KasmVNC/releases/download/${KVER}/kasmvncserver_noble_${KVER#v}_amd64.deb" -o /tmp/kasmvnc.deb \
    && apt-get install -y /tmp/kasmvnc.deb \
    && rm -f /tmp/kasmvnc.deb \
    # KasmVNC loads the snakeoil cert even with require_ssl:false; generate it world-readable.
    && make-ssl-cert generate-default-snakeoil --force-overwrite \
    && chmod 0755 /etc/ssl/private \
    && chmod 0644 /etc/ssl/private/ssl-cert-snakeoil.key \
    && rm -rf /var/lib/apt/lists/*

# Seed default XFCE look (Arc-Dark + Papirus + fonts) for new users via /etc/skel.
COPY desktop/xfce/ /etc/skel/.config/xfce4/xfconf/xfce-perchannel-xml/
# KasmVNC system defaults (plain HTTP; the per-session port is written by workspace-desktop).
COPY desktop/kasmvnc.yaml /etc/kasmvnc/kasmvnc.yaml

# Zed editor — option B: run natively inside the KasmVNC desktop. The container has no
# GPU, so Zed renders via software Vulkan (lavapipe) with the emulated-GPU override.
# (Option A — local Zed over SSH, native GPU — is enabled via `start-ssh`.)
ENV ZED_ALLOW_EMULATED_GPU=1
RUN curl -fsSL https://zed.dev/api/releases/stable/latest/zed-linux-x86_64.tar.gz -o /tmp/zed.tgz \
    && tar -xzf /tmp/zed.tgz -C /opt \
    && ln -sf /opt/zed.app/bin/zed /usr/local/bin/zed \
    && cp /opt/zed.app/share/applications/dev.zed.Zed.desktop /usr/share/applications/ \
    && sed -i 's|^Exec=.*|Exec=/usr/local/bin/zed %U|' /usr/share/applications/dev.zed.Zed.desktop \
    && rm -f /tmp/zed.tgz

# --- Python infrastructure venv (PEP 668: system pip is blocked) -----------
# The Jupyter/JupyterHub/code-server-proxy stack lives in an isolated venv; users
# keep the system clean and build their own envs with uv.
COPY requirements.txt /tmp/requirements.txt
RUN python3 -m venv /opt/jupyter \
    && /opt/jupyter/bin/pip install --no-cache-dir --upgrade pip setuptools wheel \
    && /opt/jupyter/bin/pip install --no-cache-dir -r /tmp/requirements.txt
ENV PATH="/opt/jupyter/bin:$PATH"

# JupyterHub proxy (node)
RUN npm install -g configurable-http-proxy \
    && rm -rf ~/.npm

# --- code-server (VS Code in the browser) ----------------------------------
COPY codeserver-proxy codeserver-proxy
RUN curl -fsSL https://code-server.dev/install.sh \
    | sh -s -- --version 4.122.1 \
    && pip install --no-cache-dir ./codeserver-proxy

# Default code-server editor settings for new users (/etc/skel) and root.
COPY code-server/settings.json /etc/skel/.local/share/code-server/User/settings.json
RUN mkdir -p /root/.local/share/code-server/User \
    && cp /etc/skel/.local/share/code-server/User/settings.json /root/.local/share/code-server/User/settings.json

# jupyter-server-proxy entry: a "Desktop" launcher button wired to KasmVNC.
COPY jupyter/jupyter_server_config.py /opt/jupyter/etc/jupyter/jupyter_server_config.py
# JupyterHub config (allow_all for 5.x — see the file). Passed via --config in the CMD.
COPY jupyter/jupyterhub_config.py /etc/jupyterhub/jupyterhub_config.py

# --- VS Code / Cursor CLIs + remote tunnels --------------------------------
RUN curl -Lk 'https://code.visualstudio.com/sha/download?build=stable&os=cli-alpine-x64' --output vscode_cli.tar.gz \
    && tar -xf vscode_cli.tar.gz \
    && cp ./code /usr/local/bin \
    && rm -rf vscode_cli.tar.gz

RUN curl -Lk 'https://api2.cursor.sh/updates/download-latest?os=cli-alpine-x64' --output cursor_cli.tar.gz \
    && tar -xf cursor_cli.tar.gz \
    && cp ./cursor /usr/local/bin \
    && rm -rf cursor_cli.tar.gz

COPY vscode-tunnel vscode-tunnel
RUN mkdir /vscode-tunnel/logs \
    && chmod -R 777 vscode-tunnel
RUN cp vscode-tunnel/scripts/start.sh /usr/bin/start-tunnel \
    && cp vscode-tunnel/scripts/stop.sh /usr/bin/stop-tunnel \
    && cp vscode-tunnel/scripts/refresh.sh /usr/bin/restart-tunnel \
    && cp vscode-tunnel/scripts/status.sh /usr/bin/tunnel-status \
    && chmod +x /usr/bin/start-tunnel /usr/bin/stop-tunnel /usr/bin/restart-tunnel /usr/bin/tunnel-status

# Standalone desktop launcher (Jupyter-independent path)
COPY desktop/start-desktop.sh /usr/bin/start-desktop
RUN chmod +x /usr/bin/start-desktop

# SSH server launcher for Zed remoting (option A)
COPY ssh/start-ssh.sh /usr/bin/start-ssh
RUN chmod +x /usr/bin/start-ssh

# It's tradition. A faithful Doom (chocolate-doom) with free WADs — playable in the
# desktop (menu: Games > Doom) or via `doom` in a terminal. Yes, it runs Doom.
RUN apt-get update \
    && apt-get install -y --no-install-recommends chocolate-doom freedoom \
    && rm -rf /var/lib/apt/lists/* \
    && printf '%s\n' \
       '#!/bin/sh' \
       '# Doom is a GUI app: render into the running KasmVNC desktop (display :1 by default,' \
       '# override with DISPLAY=...). Force X11 + a valid XDG_RUNTIME_DIR so SDL does not fall' \
       '# back to Wayland and abort; silence audio (no sound card in the pod).' \
       'export DISPLAY="${DISPLAY:-:1}"' \
       'export SDL_VIDEODRIVER="${SDL_VIDEODRIVER:-x11}"' \
       'export SDL_AUDIODRIVER="${SDL_AUDIODRIVER:-dummy}"' \
       'export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/tmp/runtime-$(id -u)}"' \
       'mkdir -p "$XDG_RUNTIME_DIR" 2>/dev/null && chmod 700 "$XDG_RUNTIME_DIR" 2>/dev/null' \
       'exec /usr/games/chocolate-doom -iwad /usr/share/games/doom/freedoom2.wad "$@"' \
       > /usr/local/bin/doom \
    && chmod +x /usr/local/bin/doom \
    && printf '%s\n' \
       '[Desktop Entry]' \
       'Name=Doom' \
       'Comment=Can it run Doom? Yes.' \
       'Exec=/usr/local/bin/doom' \
       'Icon=application-x-executable' \
       'Terminal=false' \
       'Type=Application' \
       'Categories=Game;ActionGame;' \
       > /usr/share/applications/doom.desktop

# Homebrew (Linux) — rootless self-service package manager. `setup-brew` installs it into
# the user's persistent home (~/.linuxbrew); new shells auto-load it. procps + file are
# Homebrew's required Linux deps (build-essential/curl/git are already present).
RUN apt-get update \
    && apt-get install -y --no-install-recommends procps file \
    && rm -rf /var/lib/apt/lists/*
COPY brew/setup-brew.sh /usr/bin/setup-brew
RUN chmod +x /usr/bin/setup-brew \
    && for rc in /etc/bash.bashrc /etc/zsh/zshrc; do \
         echo '[ -x "$HOME/.linuxbrew/bin/brew" ] && eval "$("$HOME/.linuxbrew/bin/brew" shellenv)"' >> "$rc"; \
       done

# --- French i18n + UI modernization extras ---------------------------------
# Locales + French translations (desktop, Firefox, JupyterLab), a modern GTK/xfwm4 theme
# (Materia-dark, set in desktop/xfce/*.xml), DMZ cursor, and colour-emoji font. Default
# language is ENGLISH; French is fully installed and switchable (export LANG=fr_FR.UTF-8,
# and JupyterLab → Settings → Language). To add a language: install language-pack-<xx> and
# enable <xx>_<CC>.UTF-8 in /etc/locale.gen.
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
       locales language-pack-fr language-pack-gnome-fr firefox-l10n-fr \
       materia-gtk-theme dmz-cursor-theme fonts-noto-color-emoji \
    && rm -rf /var/lib/apt/lists/* \
    && sed -i 's/^# *\(en_US.UTF-8 UTF-8\)/\1/; s/^# *\(fr_FR.UTF-8 UTF-8\)/\1/' /etc/locale.gen \
    && locale-gen \
    && update-locale LANG=en_US.UTF-8 LANGUAGE=en_US:en \
    && /opt/jupyter/bin/pip install --no-cache-dir jupyterlab-language-pack-fr-FR
ENV LANG=en_US.UTF-8 LANGUAGE=en_US:en

# Launchers: open JupyterLab / code-server / VS Code tunnel from inside the desktop
# (menu: Development). The tunnel opens in a terminal so its device-login link is visible.
COPY desktop/start-jupyterlab.sh /usr/local/bin/start-jupyterlab
COPY desktop/start-code-server.sh /usr/local/bin/start-code-server
COPY desktop/start-tunnel-desktop.sh /usr/local/bin/start-tunnel-desktop
RUN chmod +x /usr/local/bin/start-jupyterlab /usr/local/bin/start-code-server /usr/local/bin/start-tunnel-desktop \
    && printf '%s\n' '[Desktop Entry]' 'Name=JupyterLab' 'Comment=Launch JupyterLab in the browser' \
       'Exec=/usr/local/bin/start-jupyterlab' 'Icon=application-x-executable' 'Terminal=false' \
       'Type=Application' 'Categories=Development;' > /usr/share/applications/jupyterlab.desktop \
    && printf '%s\n' '[Desktop Entry]' 'Name=VS Code (code-server)' 'Comment=Launch code-server in the browser' \
       'Exec=/usr/local/bin/start-code-server' 'Icon=application-x-executable' 'Terminal=false' \
       'Type=Application' 'Categories=Development;' > /usr/share/applications/code-server.desktop \
    && printf '%s\n' '[Desktop Entry]' 'Name=VS Code Tunnel' 'Comment=Start the VS Code remote tunnel (connect from local VS Code or vscode.dev)' \
       'Exec=xfce4-terminal --title=VS-Code-Tunnel -x /usr/local/bin/start-tunnel-desktop code' \
       'Icon=application-x-executable' 'Terminal=false' \
       'Type=Application' 'Categories=Development;' > /usr/share/applications/vscode-tunnel.desktop

# --- Neon-cyber desktop theme (Tokyo Night) + desktop launchers --------------------------
# Tokyo Night GTK + xfwm4 theme (built from source; matches the Omarchy aesthetic) plus its
# neon-cyan icon set. Applied to XFCE via desktop/xfce/*.xml. (KDE Plasma was removed — it
# can't start on this GPU-less Xvnc.)
RUN apt-get update \
    && apt-get install -y --no-install-recommends git sassc \
    && rm -rf /var/lib/apt/lists/* \
    && git clone --depth=1 https://github.com/Fausto-Korpsvart/Tokyo-Night-GTK-Theme /tmp/tn \
    && /tmp/tn/themes/install.sh -d /usr/share/themes -c dark \
    && cp -r /tmp/tn/icons/Tokyonight-Dark-Cyan /usr/share/icons/ \
    && gtk-update-icon-cache -f /usr/share/icons/Tokyonight-Dark-Cyan 2>/dev/null || true \
    && rm -rf /tmp/tn

# JupyterLab + code-server + VS Code tunnel as DESKTOP icons (double-click on the desktop),
# so you can jump into "JupyterLab mode" or start a tunnel from the desktop — in addition to
# the app menu entries.
RUN mkdir -p /etc/skel/Desktop \
    && cp /usr/share/applications/jupyterlab.desktop \
          /usr/share/applications/code-server.desktop \
          /usr/share/applications/vscode-tunnel.desktop /etc/skel/Desktop/ \
    && chmod +x /etc/skel/Desktop/*.desktop

# Default wallpaper: a Tokyo Night radial gradient, generated at build (imagemagick is used
# then purged so it doesn't bloat the image). Applied per-session by set-wallpaper (XFCE
# autostart) because the VNC monitor/output name varies.
RUN apt-get update \
    && apt-get install -y --no-install-recommends imagemagick \
    && mkdir -p /usr/share/backgrounds \
    && convert -size 1920x1080 radial-gradient:'#24283b'-'#16161e' /usr/share/backgrounds/tokyonight.png \
    && apt-get purge -y imagemagick && apt-get autoremove -y \
    && rm -rf /var/lib/apt/lists/*
COPY desktop/set-wallpaper.sh /usr/local/bin/set-wallpaper
COPY desktop/autostart/set-wallpaper.desktop /etc/skel/.config/autostart/set-wallpaper.desktop
RUN chmod +x /usr/local/bin/set-wallpaper

# Passwordless sudo for the (dynamically-created) workspace user, so they can install
# OS packages. Single-tenant pod -> granting all users is acceptable. NOTE: runtime
# apt installs live on the EPHEMERAL container fs (only /home/<user> persists) — bake
# permanent packages into this Dockerfile, or use mise/uv for persistent user tooling.
RUN echo 'ALL ALL=(ALL) NOPASSWD:ALL' > /etc/sudoers.d/workspace \
    && chmod 0440 /etc/sudoers.d/workspace \
    && visudo -cf /etc/sudoers.d/workspace

# Latest security/patch updates for the 24.04 base. Kept late so it doesn't invalidate the
# heavy base layers. (24.04 LTS is current/supported to 2029+; 26.04 exists but moving to it
# is a separate migration — KasmVNC has no 26.04 .deb yet and 26.04 ships Python 3.13.)
RUN apt-get update && apt-get upgrade -y && rm -rf /var/lib/apt/lists/*

# --- Git config (system-wide, applies to every user) -----------------------
RUN git config --system credential.helper store \
    && git config --system core.filemode false \
    && git config --system core.pager delta \
    && git config --system interactive.diffFilter 'delta --color-only' \
    && git config --system delta.navigate true \
    && git config --system delta.line-numbers true \
    && git config --system merge.conflictstyle zdiff3 \
    && git config --system init.defaultBranch main \
    && git config --system pull.rebase true \
    && git config --system push.autoSetupRemote true

# Force shell to bash as the Nimbus platform isn't respecting JupyterHub option
RUN ln -sf /bin/bash /bin/sh

# THE login XSRF 403 fix. JupyterHub hashes the anonymous (login-page) XSRF token from the
# client IP; behind Istio that IP varies between the GET (form) and POST (submit) -> token
# mismatch -> 403 "XSRF cookie does not match POST argument". This env var (read only by
# JupyterHub >=5.3, hence the version bump) collapses ALL IPs — both IPv4 and IPv6 — to a
# constant, so the token no longer depends on the proxy IP. cookie+token XSRF still applies.
ENV JUPYTERHUB_XSRF_ANONYMOUS_IP_CIDRS="0.0.0.0/0;::/0"

# Desktop-first entrypoint (KasmVNC at the root; DE selectable via WORKSPACE_DE). Kept last
# so iterating on this script doesn't invalidate the heavy desktop/KDE layers above.
COPY desktop/workspace-desktop.sh /usr/bin/workspace-desktop
RUN chmod +x /usr/bin/workspace-desktop

# Expose port and set the default command.
#   8000 = the workspace port Istio routes to (here: the KasmVNC desktop).
EXPOSE 8000
# DESKTOP-FIRST — KasmVNC XFCE is the entrypoint, served at the root on 8000. No JupyterHub
# login => the XSRF 403 cannot occur; the websocket works (no URL prefix). You authenticate
# to KasmVNC with your workspace credentials. JupyterLab / code-server / Zed are launched
# from inside the desktop. To revert to Jupyter-first, restore the jupyterhub CMD:
#   CMD ["jupyterhub", "--config=/etc/jupyterhub/jupyterhub_config.py",
#        "--Spawner.default_url=/lab", "--Spawner.http_timeout=90",
#        "--NotebookApp.terminado_settings={'shell_command':'[/bin/bash]'}"]
CMD ["/usr/bin/workspace-desktop"]
