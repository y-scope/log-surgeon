parent_path=$( cd "$(dirname "${BASH_SOURCE[0]}")" ; pwd -P )
cd "$parent_path/.."
ln -s tools/yscope-dev-utils/lint-configs/.clang-tidy .clang-tidy
