{
  description = "nix_stack - reproducible multi-language dev environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
  };

  outputs = { self, nixpkgs }:
    let
      forAllSystems = nixpkgs.lib.genAttrs [
        "x86_64-linux"   # Intel/AMD Linux
        "aarch64-linux"  # ARM Linux (e.g. Graviton CI, Asahi)
        "x86_64-darwin"  # Intel Mac
        "aarch64-darwin" # Apple Silicon Mac
      ];
      pkgsFor = system: import nixpkgs { inherit system; };
    in
    {
      devShells = forAllSystems (system:
        let pkgs = pkgsFor system; in
        {
          default = pkgs.mkShell {
            buildInputs = [
              # Rust
              pkgs.rustc
              pkgs.cargo
              pkgs.rust-analyzer

              # Node.js
              pkgs.nodejs_26

              # Python
              pkgs.python314
              pkgs.uv

              # Utilities
              pkgs.jq
              pkgs.curl
            ];

            shellHook = ''
              echo "nix_stack dev shell activated"
              echo "  rustc:  $(rustc --version)"
              echo "  node:   $(node --version)"
              echo "  python: $(python3 --version)"
              echo "  uv:     $(uv --version)"

              # ── Bootstrap: make the code ready to run, not just the tools ──
              # The toolchains above come from Nix; project dependencies do not.
              # Sync them here so a fresh checkout is immediately runnable with
              # `uv run nix-stack dev` (which itself does no install step).

              # Python: uv sync is idempotent and near-instant when up to date.
              echo "  syncing python deps (uv sync)..."
              uv sync --quiet

              # Frontend: only install when node_modules is absent, to keep
              # every direnv re-entry fast. Delete the dir to force a reinstall.
              if [ ! -d frontend/node_modules ]; then
                echo "  installing frontend deps (npm install)..."
                ( cd frontend && npm install --silent )
              fi

              echo "Ready. Run: uv run nix-stack dev"
            '';
          };
        }
      );

      packages = forAllSystems (system:
        let pkgs = pkgsFor system; in
        {
          # The compiled Rust binary. The production image is built from the
          # Dockerfile (static musl binary → scratch), not from Nix — see the
          # Dockerfile. Nix's job here is the reproducible dev shell above.
          backend = pkgs.rustPlatform.buildRustPackage {
            pname = "nix-stack-backend";
            version = "0.1.0";
            src = ./backend;
            cargoLock.lockFile = ./backend/Cargo.lock;
          };
        }
      );
    };
}
