#!/usr/bin/env bash
# Starts a second, isolated development device for local pairing/sync tests.
# Keep the first instance on the normal profile and port 1420.

set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
profile_root="$project_root/.tauri-test-instance"

# Tauri resolves its app-data location from these Linux XDG directories.
# Giving this process its own profile creates a separate SQLite database,
# identity.key, settings, and cached state—equivalent to a fresh computer.
export XDG_DATA_HOME="$profile_root/data"
export XDG_CONFIG_HOME="$profile_root/config"
export XDG_CACHE_HOME="$profile_root/cache"

mkdir -p "$XDG_DATA_HOME" "$XDG_CONFIG_HOME" "$XDG_CACHE_HOME" "$project_root/logs"

cd "$project_root"
exec cargo tauri dev --config src-tauri/tauri.second.conf.json
