#!/usr/bin/env bash

set -euo pipefail

flyctl deploy --ha=false --now
