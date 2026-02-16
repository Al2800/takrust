#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
COMPOSE_FILE="${SCRIPT_DIR}/docker-compose.tak-server-smoke.yml"

if ! command -v docker >/dev/null 2>&1; then
  echo "docker is required for TAK Server smoke harness" >&2
  exit 1
fi

if [[ -z "${RUSTAK_TAK_SERVER_IMAGE:-}" ]]; then
  echo "set RUSTAK_TAK_SERVER_IMAGE to a TAK Server docker image before running this harness" >&2
  exit 1
fi

export RUSTAK_TAK_SERVER_STREAM_HOST="${RUSTAK_TAK_SERVER_STREAM_HOST:-127.0.0.1}"
export RUSTAK_TAK_SERVER_STREAM_PORT="${RUSTAK_TAK_SERVER_STREAM_PORT:-8089}"
export RUSTAK_TAK_SERVER_STREAM_PATH="${RUSTAK_TAK_SERVER_STREAM_PATH:-/Marti/api/channels/streaming}"
export RUSTAK_RUN_TAK_SERVER_SMOKE=1

cleanup() {
  docker compose -f "${COMPOSE_FILE}" down -v >/dev/null 2>&1 || true
}
trap cleanup EXIT

docker compose -f "${COMPOSE_FILE}" up -d

echo "waiting for ${RUSTAK_TAK_SERVER_STREAM_HOST}:${RUSTAK_TAK_SERVER_STREAM_PORT}..."
ready=0
for _attempt in $(seq 1 30); do
  if (echo >"/dev/tcp/${RUSTAK_TAK_SERVER_STREAM_HOST}/${RUSTAK_TAK_SERVER_STREAM_PORT}") >/dev/null 2>&1; then
    ready=1
    break
  fi
  sleep 2
done

if [[ "${ready}" -ne 1 ]]; then
  echo "TAK Server smoke endpoint did not become reachable in time" >&2
  exit 1
fi

cd "${REPO_ROOT}"
cargo test --manifest-path tests/interop_harness/Cargo.toml --test tak_server_docker_smoke -- --nocapture
