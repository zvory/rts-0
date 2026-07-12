#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  scripts/fly-logs.sh [beta|mainline|app-name] [recent|tail] [fly logs flags...]
  scripts/fly-logs.sh [beta|mainline|app-name] search --from ISO8601 [--to ISO8601] [options]

Examples:
  scripts/fly-logs.sh beta recent
  scripts/fly-logs.sh mainline recent
  scripts/fly-logs.sh beta tail --region ewr
  scripts/fly-logs.sh beta search --from 2026-06-11T22:00:00Z --to 2026-06-11T23:30:00Z
  scripts/fly-logs.sh beta search --from 2026-06-11T22:00:00Z --filter 'performance tick summary|client network report'

Search mode uses Fly's HTTP logs API, which can page through the historical retention window.
Search options:
  --from ISO8601      Start timestamp, inclusive. Required for search mode.
  --to ISO8601        End timestamp, inclusive. Defaults to now.
  --filter REGEX      Local jq regex matched against message text.
  --region REGION     Restrict to a Fly region, for example ewr.
  --instance ID       Restrict to one Machine instance.
  --max-pages N       Stop after N API pages. Defaults to 200.

Reads FLY_API_TOKEN from the environment, this worktree's .env, or the main worktree's .env.
EOF
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

channel="${1:-beta}"
mode="${2:-recent}"
if [[ $# -gt 0 ]]; then
  shift
fi
if [[ $# -gt 0 ]]; then
  shift
fi

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
env_files=("$repo_root/.env")

main_worktree="$(
  git worktree list --porcelain 2>/dev/null \
    | awk '
      /^worktree / { path = substr($0, 10) }
      /^branch refs\/heads\/main$/ { print path; exit }
    '
)"
if [[ -n "$main_worktree" && "$main_worktree" != "$repo_root" ]]; then
  env_files+=("$main_worktree/.env")
fi

if [[ -z "${FLY_API_TOKEN:-}" ]]; then
  for env_file in "${env_files[@]}"; do
    if [[ -f "$env_file" ]]; then
      token="$(
        sed -nE 's/^(export[[:space:]]+)?FLY_API_TOKEN=(.*)$/\2/p' "$env_file" \
          | tail -n 1
      )"
      token="${token#\"}"
      token="${token%\"}"
      token="${token#\'}"
      token="${token%\'}"
      if [[ -n "$token" ]]; then
        export FLY_API_TOKEN="$token"
        break
      fi
    fi
  done
fi

if [[ -z "${FLY_API_TOKEN:-}" ]]; then
  for env_file in "${env_files[@]}"; do
    if [[ -f "$env_file" ]]; then
      env_hint="$env_file"
      break
    fi
  done
fi

if [[ -z "${FLY_API_TOKEN:-}" ]]; then
  echo "error: FLY_API_TOKEN is not set and was not found in ${env_hint:-.env}" >&2
  exit 2
fi

case "$channel" in
  mainline|main|prod|production)
    app="${FLY_MAINLINE_APP:-rts-0-zvorygin}"
    ;;
  beta)
    app="${FLY_BETA_APP:-rts-0-zvorygin-beta}"
    ;;
  *)
    app="$channel"
    ;;
esac

iso_to_ns() {
  local value="$1"
  node -e '
    const value = process.argv[1];
    const ms = Date.parse(value);
    if (!Number.isFinite(ms)) {
      console.error(`error: invalid ISO8601 timestamp: ${value}`);
      process.exit(2);
    }
    console.log((BigInt(ms) * 1000000n).toString());
  ' "$value"
}

search_logs() {
  local from="" to="" filter="" region="" instance="" max_pages=200

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --from)
        from="${2:-}"
        shift 2
        ;;
      --to)
        to="${2:-}"
        shift 2
        ;;
      --filter)
        filter="${2:-}"
        shift 2
        ;;
      --region)
        region="${2:-}"
        shift 2
        ;;
      --instance|--machine)
        instance="${2:-}"
        shift 2
        ;;
      --max-pages)
        max_pages="${2:-}"
        shift 2
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      *)
        echo "error: unknown search option '$1'" >&2
        usage >&2
        exit 2
        ;;
    esac
  done

  if [[ -z "$from" ]]; then
    echo "error: search mode requires --from ISO8601" >&2
    usage >&2
    exit 2
  fi
  if ! [[ "$max_pages" =~ ^[0-9]+$ ]] || [[ "$max_pages" -lt 1 ]]; then
    echo "error: --max-pages must be a positive integer" >&2
    exit 2
  fi

  local next_token end_ns page previous_token response
  local -a curl_args
  next_token="$(iso_to_ns "$from")"
  if [[ -n "$to" ]]; then
    end_ns="$(iso_to_ns "$to")"
  else
    end_ns="$(node -e 'console.log((BigInt(Date.now()) * 1000000n).toString())')"
  fi

  for ((page = 1; page <= max_pages; page++)); do
    previous_token="$next_token"
    curl_args=(
      -fsS
      -G
      -H "Authorization: $FLY_API_TOKEN"
      --data-urlencode "next_token=$next_token"
    )
    if [[ -n "$region" ]]; then
      curl_args+=(--data-urlencode "region=$region")
    fi
    if [[ -n "$instance" ]]; then
      curl_args+=(--data-urlencode "instance=$instance")
    fi
    response="$(
      curl "${curl_args[@]}" "https://api.fly.io/api/v1/apps/$app/logs"
    )"

    jq -c --arg end_ns "$end_ns" --arg filter "$filter" '
      def timestamp_ns:
        capture("^(?<base>[^.Z]+)(?:\\.(?<frac>[0-9]+))?Z$")
        | (((.base + "Z" | fromdateiso8601) * 1000000000) | floor)
          + (((.frac // "0")[0:9] + "000000000")[0:9] | tonumber);
      .data[]
      | .attributes as $a
      | (($a.timestamp | timestamp_ns) | tostring) as $ts_ns
      | select(($ts_ns | tonumber) <= ($end_ns | tonumber))
      | {
          level: $a.level,
          instance: $a.instance,
          message: $a.message,
          region: $a.region,
          timestamp: $a.timestamp,
          meta: $a.meta
        }
      | select($filter == "" or (.message // "" | test($filter; "i")))
    ' <<<"$response"

    next_token="$(jq -r '.meta.next_token // empty' <<<"$response")"
    if [[ -z "$next_token" || "$next_token" == "$previous_token" ]]; then
      break
    fi

    if ! jq -e --arg end_ns "$end_ns" '
      def timestamp_ns:
        capture("^(?<base>[^.Z]+)(?:\\.(?<frac>[0-9]+))?Z$")
        | (((.base + "Z" | fromdateiso8601) * 1000000000) | floor)
          + (((.frac // "0")[0:9] + "000000000")[0:9] | tonumber);
      [.data[].attributes.timestamp
        | ((timestamp_ns) | tostring)
        | select((. | tonumber) > ($end_ns | tonumber))]
      | length == 0
    ' >/dev/null <<<"$response"; then
      break
    fi
  done
}

if [[ "$mode" == "search" ]]; then
  search_logs "$@"
  exit 0
fi

args=(logs -a "$app" --json)
case "$mode" in
  recent)
    args+=(--no-tail)
    ;;
  tail)
    ;;
  *)
    echo "error: unknown mode '$mode' (expected recent, tail, or search)" >&2
    usage >&2
    exit 2
    ;;
esac

exec flyctl "${args[@]}" "$@"
