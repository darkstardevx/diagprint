#!/usr/bin/env bash
set -euo pipefail

mode="${1:-fast}"
cd "$(git rev-parse --show-toplevel)"

is_code_path() {
  case "$1" in
    *.rs|Cargo.toml|Cargo.lock|rustfmt.toml|clippy.toml|.clippy.toml|*/Cargo.toml) return 0 ;;
    *) return 1 ;;
  esac
}

approved_plan_from_head() {
  git cat-file -e HEAD:.plans/ACTIVE 2>/dev/null || {
    echo "ERROR: Rust changes require an Approved plan already committed in HEAD." >&2
    return 1
  }

  local active status
  active="$(git show HEAD:.plans/ACTIVE | tr -d '\r\n')"

  [[ "$active" == .plans/*.plan.md ]] || {
    echo "ERROR: invalid active plan path: $active" >&2
    return 1
  }

  git cat-file -e "HEAD:$active" 2>/dev/null || {
    echo "ERROR: active plan missing from HEAD: $active" >&2
    return 1
  }

  status="$(git show "HEAD:$active" | grep '^Status:' | head -n1 || true)"
  [[ "$status" == "Status: Approved" ]] || {
    echo "ERROR: active plan is not Approved: ${status:-<missing>}" >&2
    return 1
  }

  if git diff --cached --name-only -- .plans/ACTIVE "$active" | grep -q .; then
    echo "ERROR: plan changes and Rust implementation cannot be committed together." >&2
    return 1
  fi

  printf '%s\n' "$active"
}

precommit() {
  git diff --cached --check

  local staged
  staged="$(git diff --cached --name-only --diff-filter=ACMR)"
  [[ -n "$staged" ]] || exit 0

  local has_code=0
  local has_rust=0

  while IFS= read -r path; do
    [[ -n "$path" ]] || continue
    is_code_path "$path" && has_code=1
    [[ "$path" == *.rs ]] && has_rust=1
  done <<< "$staged"

  [[ "$has_code" -eq 1 ]] || exit 0

  local unstaged=()
  while IFS= read -r path; do
    [[ -n "$path" ]] || continue
    is_code_path "$path" && unstaged+=("$path")
  done < <(git diff --name-only)

  if [[ "${#unstaged[@]}" -gt 0 ]]; then
    echo "ERROR: unstaged Rust/Cargo changes exist:" >&2
    printf '  %s\n' "${unstaged[@]}" >&2
    exit 1
  fi

  if [[ "$has_rust" -eq 1 ]]; then
    echo "Approved committed plan: $(approved_plan_from_head)"
    cargo fmt --all -- --check
  fi

  cargo check --workspace --all-targets --all-features --locked
  cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
}

case "$mode" in
  precommit)
    precommit
    ;;
  fast)
    exec ./scripts/release-gates quick
    ;;
  forensics)
    ./scripts/release-gates quick
    for t in diagnostic_forensics history_cli git_provenance git_blame_cli; do
      cargo test --locked --test "$t"
    done
    ;;
  full)
    exec ./scripts/release-gates full
    ;;
  *)
    echo "Usage: ./scripts/gate.sh {precommit|fast|forensics|full}" >&2
    exit 2
    ;;
esac
