#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "Usage: $0 <github-repo-url> [branch]"
  echo "Example: $0 git@github.com:<user>/<repo>.git main"
  exit 1
fi

REPO_URL="$1"
BRANCH="${2:-$(git branch --show-current)}"

if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  echo "Error: not inside a git repository"
  exit 1
fi

if git remote get-url origin >/dev/null 2>&1; then
  git remote set-url origin "$REPO_URL"
else
  git remote add origin "$REPO_URL"
fi

git push -u origin "$BRANCH"

echo "Pushed branch '$BRANCH' to '$REPO_URL'."
