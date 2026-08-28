#!/usr/bin/env bash
#
# Enforces the rule this project is built around: the serving path holds no LLM
# client and opens no outbound connection.
#
# `core`, `embed`, `classify`, and `api` answer queries. If any of them can
# reach an HTTP client, a query stops being reproducible and offline — it can
# silently depend on a network call, a rate limit, or an API key. Cargo's
# dependency graph already makes that a compile-time property; this is what
# notices when someone adds the edge.
#
# The check is on the *outbound client* (`reqwest`), not on HTTP generally:
# `api` legitimately depends on hyper to serve requests. Serving is not
# calling out.

set -euo pipefail

# Crates that answer a query. Nothing here may reach an outbound HTTP client.
SERVING=(torimemo-core torimemo-embed torimemo-classify torimemo-api)

# Crates that may. `enrich` fetches page metadata and calls the labelling
# model; both are offline batch work that no request path waits on.
OUTBOUND=(torimemo-enrich)

status=0

for crate in "${SERVING[@]}"; do
    # `--no-default-features` is the configuration CI tests, and the one where
    # the claim must hold without qualification.
    if cargo tree -p "$crate" --no-default-features -e normal 2>/dev/null | grep -q "reqwest"; then
        echo "FAIL: $crate can reach reqwest; the serving path must not make outbound calls"
        cargo tree -p "$crate" --no-default-features -e normal --invert reqwest 2>/dev/null | head -20
        status=1
    else
        echo "ok:   $crate has no outbound HTTP client"
    fi
done

# The dependency that would most easily go unnoticed: `api` gaining a path to
# `enrich`, which would put a labelling model in the request path.
for forbidden in torimemo-enrich; do
    if cargo tree -p torimemo-api --no-default-features -e normal 2>/dev/null | grep -q "$forbidden"; then
        echo "FAIL: torimemo-api depends on $forbidden; the serving path must not reach the model layer"
        status=1
    else
        echo "ok:   torimemo-api does not depend on $forbidden"
    fi
done

# The inverse, so this script cannot pass by checking nothing: the crates that
# are *supposed* to reach the network must still do so. A rename or a refactor
# that silently drops the dependency would otherwise turn every check above
# green for the wrong reason.
for crate in "${OUTBOUND[@]}"; do
    if cargo tree -p "$crate" -e normal 2>/dev/null | grep -q "reqwest"; then
        echo "ok:   $crate reaches the network, as intended"
    else
        echo "FAIL: $crate no longer depends on reqwest; this check is not testing what it claims"
        status=1
    fi
done

exit "$status"
