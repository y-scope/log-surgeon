#!/usr/bin/env bash

# Exit on any error
set -e

# Error on undefined variable
set -u

repo_dir="$(git rev-parse --show-toplevel)"
clang_tidy_config_src_relative_to_repo="tools/yscope-dev-utils/lint-configs/.clang-tidy"

clang_tidy_config_src="${repo_dir}/${clang_tidy_config_src_relative_to_repo}"
clang_tidy_config_dst="$repo_dir/.clang-tidy"
if [ ! -e "$clang_tidy_config_dst" ]; then
    ln -s "$clang_tidy_config_src_relative_to_repo" "$clang_tidy_config_dst"
else
    if [ "$(readlink -f "$clang_tidy_config_dst")" != "$(readlink -f "$clang_tidy_config_src")" ];
    then
        echo "Unexpected clang-tidy config at $clang_tidy_config_dst"
    fi
fi
