# Workflow image for the `extract` pipeline (hierarchy -> condense).
# Built and registered on the Nimbus tenant by `uv run extract deploy`, which
# calls the vendored deploy client in extract.deploy. It sits at the project
# root alongside pyproject + uv.lock + src, which is the build context, so
# `COPY . .` and `uv sync` see the whole project (deploy client included).
#
# This is a CPU image: torch installs its CPU wheel and a small Qwen model runs
# on CPU. For large samples or bigger models, switch the base to an NVIDIA CUDA
# image and request a GPU instanceTypeId in the workflow step resources.
FROM python:3.12-slim-bookworm

# uv: fast, lockfile-faithful installs. Copy the static binary from the official
# image rather than curl-piping an installer.
COPY --from=ghcr.io/astral-sh/uv:latest /uv /uvx /bin/

ENV UV_LINK_MODE=copy \
    UV_COMPILE_BYTECODE=1 \
    HF_HOME=/home/nimbus-user/.cache/huggingface

WORKDIR /app

# Copy the whole context: pyproject + uv.lock + src (deploy client included).
COPY . .

# Resolve straight from the lockfile; --no-dev drops the pytest group. This
# installs torch/transformers (the heavy layer) once, cached across runs.
RUN uv sync --frozen --no-dev

# Nimbus credentials. The warehouse oauth path (database.connect_oauth) calls the
# Nimbus connections API with API_KEY as the Authorization header to fetch the
# Snowflake token at runtime — without it that call 401s. `extract deploy`
# forwards the local API_KEY as this build arg and it is baked into the image
# env. NOTE: this embeds the key in an image layer; prefer a tenant-injected
# value if your platform provides one.
ARG API_KEY
ENV API_KEY=$API_KEY

# Nimbus runs containers as NIMBUS_USER_ID; give it a writable home for the HF cache.
ARG NIMBUS_USER_ID=1000
RUN useradd -m -d /home/nimbus-user -u "$NIMBUS_USER_ID" nimbus-user \
    && chown -R nimbus-user:nimbus-user /app /home/nimbus-user
USER $NIMBUS_USER_ID

# Workflow steps override this with `uv run extract hierarchy|condense ...`;
# the default just confirms the entrypoint resolves.
CMD ["uv", "run", "extract", "--help"]
