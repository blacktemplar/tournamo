#!/bin/sh
# Runs on the host before the dev container starts (`initializeCommand`): writes .env with what
# compose.yaml needs to know about this checkout.
set -eu

cd "$(dirname "$0")/.."
workspace=$(pwd -P)
git_common_dir=$(git rev-parse --path-format=absolute --git-common-dir)
# mounted read-only, so it has to exist
mkdir -p "$git_common_dir/hooks"
hash=$(printf '%s' "$workspace" | sha256sum | cut -c1-8)

cat > .devcontainer/.env <<EOF
COMPOSE_NAME=tournamo-$hash
USER_ID=$(id -u)
WORKSPACE=$workspace
GIT_COMMON_DIR=$git_common_dir
GIT_NAME='$(git config user.name)'
GIT_EMAIL='$(git config user.email)'
EOF
