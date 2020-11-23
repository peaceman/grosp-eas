#!/usr/bin/env bash
#
# exports docker image hashes for the compile and runtime stage
# as COMPILE_IMAGE_ID and RUNTIME_IMAGE_ID

source "${BASH_SOURCE%/*}/default-vars.sh"

set -euo pipefail

COMPILE_IMAGE_ID_FILE=$(mktemp)
RUNTIME_IMAGE_ID_FILE=$(mktemp)

docker pull "$DOCKER_REPO":"compile-stage-$DOCKER_TARGET_TAG" || true
docker pull "$DOCKER_REPO":"$DOCKER_TARGET_TAG" || true

# build the compile stage
docker build --target compile-image \
    --iidfile "$COMPILE_IMAGE_ID_FILE" \
    --file .docker/Dockerfile \
    --cache-from "$DOCKER_REPO":"compile-stage-$DOCKER_TARGET_TAG" \
    .

export COMPILE_IMAGE_ID=$(cat "$COMPILE_IMAGE_ID_FILE")

# build the runtime stage, using cached compile stage
docker build --target runtime-image \
    --iidfile "$RUNTIME_IMAGE_ID_FILE" \
    --file .docker/Dockerfile \
    --cache-from "$COMPILE_IMAGE_ID" \
    --cache-from "$DOCKER_REPO":"$DOCKER_TARGET_TAG" \
    .

export RUNTIME_IMAGE_ID=$(cat "$RUNTIME_IMAGE_ID_FILE")

rm "$COMPILE_IMAGE_ID_FILE" "$RUNTIME_IMAGE_ID_FILE"
