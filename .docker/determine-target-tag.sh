#!/usr/bin/env bash
#
# determine the target tag from the current branch or tag
# supplied by the GITHUB_REV env var or default to latest
#
# exports the result as DOCKER_TARGET_TAG var

TARGET_TAG_BRANCH=""
TARGET_TAG_TAG=""

if [[ "$GITHUB_REF" == refs/heads/* ]]; then
    TARGET_TAG_BRANCH="${GITHUB_REF#refs/heads/}"
    echo "Determined the target tag from the current branch: ${TARGET_TAG_BRANCH}"
fi

if [[ "$GITHUB_REF" == refs/tags/* ]]; then
    TARGET_TAG_TAG="${GITHUB_REF#refs/tags/}"
    echo "Determined the target tag from the current tag: ${TARGET_TAG_TAG}"
fi

DOCKER_TARGET_TAG="${TARGET_TAG_BRANCH:-$TARGET_TAG_TAG}"
DOCKER_TARGET_TAG="${DOCKER_TARGET_TAG:-latest}"

export DOCKER_TARGET_TAG
