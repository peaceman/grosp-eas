#!/usr/bin/env

echo "${DOCKER_LOGIN_PASSWORD:?Missing docker login credentials}" \
    | docker login --username "${DOCKER_LOGIN_USERNAME:?Missing docker login credentials}" \
    --password-stdin
