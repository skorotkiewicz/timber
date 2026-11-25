# https://github.com/casey/just

[private]
default:
    @just --list

build:
    cargo build --release

build-all:
    cargo build --release --all-features

run *args:
    cargo run --all-features -- {{ args }}

fmt:
    cargo fmt
    cargo clippy --all-targets --all-features -- -D warnings
    @ # cargo shear --fix # cargo install shear

check:
    cargo fmt --check
    cargo clippy --all-targets --all-features -- -D warnings

test: fmt
    cargo test --all-targets --all-features

install-hook:
    @printf '#!/bin/sh\nset -e\njust check\n' > .git/hooks/pre-commit
    @chmod +x .git/hooks/pre-commit

remove-hook:
    @rm .git/hooks/pre-commit

add-tag:
    #!/usr/bin/env bash
    set -euo pipefail
    VERSION=$(grep '^version' Cargo.toml | head -1 | cut -d'"' -f2)
    git push origin dev
    git tag -a "v${VERSION}" -m "Release v${VERSION}"
    git push origin "v${VERSION}"

# `just remove-tag v0.0.0` or `just remove-tag` (uses fzf)
remove-tag VERSION="":
    #!/usr/bin/env bash
    set -euo pipefail
    tag="{{ VERSION }}"
    [ -z "$tag" ] && tag=$(git tag | sort -V | fzf --prompt="Select tag to remove: ")
    [ -z "$tag" ] && echo "No tag selected" && exit 1
    git tag -d "$tag"
    git push --delete origin "$tag"

# Undo last commit locally and on remote, keeping changes staged.
undo-commit:
    #!/usr/bin/env bash
    set -euo pipefail

    upstream=$(git rev-parse --abbrev-ref --symbolic-full-name '@{u}')
    remote="${upstream%%/*}"
    branch="${upstream#*/}"

    git reset --soft HEAD~1
    git push --force-with-lease "$remote" "HEAD:$branch"

# Full end-to-end test: encode, decode JSON, decode tree text
test-e2e: build
	rm -rf test_dir
	mkdir -p test_dir/sub
	echo "hello" > test_dir/a.txt
	echo "rust" > test_dir/sub/b.rs
	./target/release/timber encode test_dir > test_enc.json
	./target/release/timber decode test_enc.json test_decoded
	@echo "=== JSON roundtrip ==="
	@cat test_decoded/test_dir/a.txt
	@cat test_decoded/test_dir/sub/b.rs
	tree test_dir > test_tree.txt
	./target/release/timber decode test_tree.txt test_tree_decoded
	@echo "=== tree-text decode ==="
	@find test_tree_decoded | sort
	tree -J test_dir > test_tree_j.json
	./target/release/timber decode test_tree_j.json test_j_decoded
	@echo "=== tree -J decode ==="
	@find test_j_decoded | sort
	@echo "All e2e tests passed!"

# Clean up all test artifacts including test_dir
clean-e2e:
	rm -f test_enc.json test_tree.txt test_tree_j.json
	rm -rf test_decoded test_tree_decoded test_j_decoded test_dir
	@echo "E2e artifacts cleaned."
