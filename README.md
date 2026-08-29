# とりメモ (Torimemo)

Agentic, lightning-fast, lightweight AI bookmark manager with FastText classification & ONNX context comprehension.

Local-first bookmark capture and recall. One SQLite file, one binary, no
network at query time.

## The idea

A **capture** is an immutable record that a link arrived — from Telegram, from
the browser, from a vault backfill. A **bookmark** is the deduplicated resource
those captures point at. Sending yourself the same link twice produces one
bookmark and two captures: the duplicate leaves your view while the fact that
you saved it twice is kept, and that repeat count is the strongest available
signal that a link actually mattered.

## Architecture

The workspace enforces one rule structurally: **the serving path depends on no
model SDK and no network client.** `core`, `embed`, and `api` never list
`enrich` as a dependency, so the boundary is a compile error rather than a
convention.

```
core     normalize -> dedupe -> store        deterministic, no model
embed    local ONNX vectors + cosine recall  in-process, no network
classify the distilled tag model             trained offline, served locally
api      HTTP surface + /v1/tools registry   serving path, loopback only
enrich   page metadata + model labelling     the only crate allowed the network
capture  telegram bot, ingest endpoints      (pending)
```

Every model-produced value carries the model version that produced it and a
hash of its input, so re-embedding with a newer model is a diff rather than a
silent overwrite.

## Use

```sh
cargo build --release

torimemo import-vault ~/Documents/vault/ALL\ BOOKMARKS
torimemo import-browser ~/Downloads/bookmarks.html
torimemo import-stars <github-user>  # starred repos; re-run to pick up new ones
torimemo add https://example.com --context "why I saved it"

torimemo enrich                      # fetch titles; resumable, per-host polite
torimemo label                       # tag with Haiku (needs ANTHROPIC_API_KEY)
torimemo label --labeller rules      # keyword baseline; no key, no network
torimemo embed                       # backfill vectors; resumable

torimemo prune                       # review what is no longer worth keeping
torimemo prune --reason all --apply  # and remove it

torimemo tags                        # tag counts across the corpus
torimemo export-training --out t.jsonl      # training set as JSON Lines
torimemo train                       # fit the local classifier, print held-out scores
torimemo suggest <url>               # predict tags locally, no network
torimemo recall "container orchestration"   # semantic
torimemo search "kubernetes"                # lexical, FTS5

torimemo serve                       # HTTP API on 127.0.0.1:7645
torimemo token issue --name odin     # mint a bearer token (shown once)
torimemo stats
torimemo repeats                     # what you saved more than once
```

`--model deterministic` swaps the ONNX model for hash vectors: no download, no
network, lexical overlap only. CI runs against it.

## API

Loopback only, no auth — the socket is the access control, and the intended
consumer is the user's own agent on the same machine.

| | |
|---|---|
| `POST /captures` | save a link; idempotent on the canonical URL |
| `GET /recall?q=` | semantic search, scored |
| `GET /search?q=` | lexical search |
| `GET /bookmarks/{id}` | a bookmark and every capture of it |
| `POST /events` | record an interaction (the ranking signal) |
| `GET /stats` | corpus counts |

## GitHub stars

Stars are a better record of "repositories I care about" than bookmarks: they
are maintained where you press the button, they carry the repo's own
description and language, and re-running the import picks up everything
starred since last time. Archived repos and forks are skipped by default.

The description GitHub returns seeds the bookmark's title, so an imported star
is searchable immediately without the enrichment pass having to fetch anything
— but only where no title is set, since a fetched or human title is better
evidence than the API's summary.

Unauthenticated requests are limited to 60 per hour; pass `--token` (or set
`GITHUB_TOKEN`) to raise that to 5000.

## Pruning

A decade of saving leaves a lot that was never worth keeping and more that
stopped being: pages that 404, login walls that reveal nothing, postings for
roles long filled, listings for things long sold. `prune` names those
categories so a cull is a reviewable list of reasons rather than a judgement
per row, and **nothing is deleted without `--apply`**.

The default runs only the categories that rest on a measured fact — `dead`,
`walled`, `untitled`, `asset`, `session`. The rest rest on an inference (that a
posting is filled, that a listing is sold) which is usually but not always
right, so enabling them with `--reason` is the operator's call.

On a real 2,101-bookmark archive the objective rules alone matched **868**, and
the full set **1,058** — about half, most of it unreachable or unreadable
rather than merely uninteresting.

| category | basis |
|---|---|
| `dead` | the server returned 404/410, or the host stopped resolving |
| `walled` | fetched, but the page was a login wall with no metadata |
| `untitled` | no title from any source; only a bare URL remains |
| `asset` | a file rather than a page |
| `session` | a login form, personal dashboard, or private-network host |
| `dead-shortlink` | a shortener that no longer resolves |
| `job-posting` · `shopping-listing` · `ephemeral` | expired by nature |
| `contentless` | the title names the site, not the page |
| `duplicate` | another bookmark has the same title; the older is kept |

Rules match whole titles, never substrings: "Features" goes, "Features of
Rust's borrow checker" stays. An earlier draft used a title-length test and
would have deleted Carbon, Lobsters, ASCIIFlow, and Markwhen for having short
names; a test now pins that.

## Teacher and student

Tagging is deliberately split. A capable model labels the corpus **once,
offline**, against a closed vocabulary (`enrich/taxonomy.rs`); those labels
export as JSON Lines and become the training set for a small local classifier
that does the work at request time. The model produces training data — it never
answers a query.

Labels are constrained twice: the tool schema's `tags` field is an `enum` over
the vocabulary, so the API rejects an invented tag, and the response is
validated against the vocabulary again on the way in. A model tag never
displaces a human one — both rows coexist, and the serving path prefers the
human tag.

A keyword baseline (`--labeller rules`) runs the same pipeline with no key and
no network, so the queue, validation, storage, and export are all testable
offline. It reaches 625 of 1,248 titled bookmarks and mislabels job postings as
`business` or `marketing`, which is a fair statement of the gap the model is
there to close.

The student is multi-label logistic regression over the 384-dimensional
embeddings — one independent binary classifier per tag, because a bookmark
genuinely carries several at once. Two things keep the reported numbers honest:
the holdout is split before any fitting, and a label with fewer than
`--min-support` positive examples is **refused** rather than fitted, because
weights from a dozen examples describe those examples and nothing else. Scores
are macro-averaged so a rare tag failing cannot hide behind a common one
succeeding.

Trained against the keyword baseline the student reaches macro F1 0.584 on
held-out data in under a second — and the shape of that result is the argument
for the model teacher: `video` scores 0.83 where the rules are reliable,
`article` scores 0.30 where they are keyword guesses, and `job-listing` has two
examples and cannot be fitted at all.

## Engineering

CI gates every push: `cargo fmt --check`, the full suite under
`--no-default-features` (so a test run needs no model download and no network),
clippy at `-D warnings` across **both** feature combinations, a build against
the declared MSRV, and `cargo audit`.

One job is specific to this codebase. `scripts/check-boundary.sh` asserts the
architectural rule from Cargo's dependency graph: no crate in the serving path
may reach an outbound HTTP client, and `api` may not reach `enrich`. It also
asserts the inverse — that `enrich` still *does* reach the network — so the
check cannot pass by silently testing nothing.

`cargo bench -p torimemo-embed` measures search rather than asserting it.
Vectors are unit-length by construction, so the hot path is a plain dot product
across eight independent accumulators — no magnitudes to recompute, and enough
parallel chains to use the vector unit. That scans at **~3.2M vectors/second**:
0.6ms at 2,000 bookmarks, 3.2ms at 10,000, 17ms at 50,000, linear.

An approximate index was measured against this, not assumed better. libSQL's
`vector_top_k` is **slower** at this corpus size (1.4ms against 0.6ms), and
where it wins at 50,000 it returned **2 of the correct top 10**. For an archive
where a query should surface the thing you actually saved, exact wins. Revisit
at roughly 200,000 bookmarks.

CI compiles the benchmarks but does not time them: timings on a shared runner
are noise, while a benchmark that stops compiling is one nobody notices is
gone.

URL canonicalization carries property tests (`crates/core/tests/`) on top of
its examples, because it is the store's identity function: every dedupe
decision and every bookmark's primary key depends on it. They cover
idempotence, that tracking parameters never survive, that meaningful ones
always do, that parameter order does not fork identity, and that no input
panics. Adding `v` to the tracking list — which would collapse all 165 YouTube
bookmarks into one — fails them with a shrunk counterexample.

## Deployment

```sh
docker build -t torimemo .

# Mint a token into the data volume first: without one the server binds
# loopback and a container is unreachable on purpose.
docker run --rm -v torimemo-data:/data torimemo \
    torimemo token issue --name odin --scope read-write

docker run -d -p 7645:7645 \
    -v torimemo-data:/data \
    -v torimemo-models:/models \
    torimemo
```

**Mount `/models`.** It is where fastembed caches the embedding model, and
without the volume the ~127MB download repeats on every start — the container
sits silent for tens of seconds before it binds, which looks exactly like a
hang. `serve` prints `loading the … embedding model` before it does, so the
wait is visible rather than mysterious.

`serve` binds `127.0.0.1` while no service token exists and `0.0.0.0` once one
does. The bind address is derived from the auth state rather than configured,
so there is no flag that opens the port without also requiring a credential —
the insecure configuration is unreachable by construction. A container needs a
token minted into its data volume before it can serve anything:

```sh
docker run --rm -v torimemo-data:/data torimemo torimemo token issue --name odin
```

The embedding model is **not** baked into the image. fastembed fetches it into
`FASTEMBED_CACHE_PATH` on first use, mounted as a volume: baking 127MB of
weights in would triple the image and pin the model version to the image tag,
so re-embedding with a newer model would need a rebuild rather than a config
change. `TORIMEMO_DB` sets the archive path.

`ghcr.io/arcbjorn/torimemo:latest` publishes only from a master commit whose CI
run already passed — a red build never becomes `:latest`.

## Status

Working: normalization, dedupe, capture/bookmark model, SQLite + FTS5 storage,
vault and Netscape import, local ONNX embeddings, cosine recall, page metadata
enrichment, model labelling with training-set export, the distilled classifier
with held-out evaluation, HTTP API, CLI.

Pending: learned ranking, revisit-probability model, Telegram bot, browser
extension.

Measured on a real 2,101-bookmark corpus: import 2,517 URLs with 0 skipped and
416 merged as duplicates; embed all 2,101 in 42s locally with
`bge-small-en-v1.5`. Enrichment classifies each URL as enriched, bare (a login
wall or an interstitial), dead, or transiently failed — only the last is
retried, so a decade of dead links costs one pass rather than every pass.
