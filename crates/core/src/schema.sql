-- torimemo schema.
--
-- Captures are append-only; bookmarks are derived. Every model-produced value
-- (embedding, tag, score) carries the model version that produced it and a
-- hash of the input it was computed from, so a model upgrade is a diff rather
-- than a silent overwrite.

CREATE TABLE IF NOT EXISTS bookmarks (
    id                 INTEGER PRIMARY KEY,
    canonical_url      TEXT    NOT NULL UNIQUE,
    domain             TEXT    NOT NULL,
    title              TEXT,
    description        TEXT,
    first_captured_at  TEXT    NOT NULL,
    last_captured_at   TEXT    NOT NULL,
    capture_count      INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE INDEX IF NOT EXISTS bookmarks_domain ON bookmarks (domain);
CREATE INDEX IF NOT EXISTS bookmarks_last_captured ON bookmarks (last_captured_at DESC);

CREATE TABLE IF NOT EXISTS captures (
    id           INTEGER PRIMARY KEY,
    bookmark_id  INTEGER NOT NULL REFERENCES bookmarks (id) ON DELETE CASCADE,
    raw_url      TEXT    NOT NULL,
    source       TEXT    NOT NULL,
    context      TEXT,
    captured_at  TEXT    NOT NULL
) STRICT;

CREATE INDEX IF NOT EXISTS captures_bookmark ON captures (bookmark_id);
CREATE INDEX IF NOT EXISTS captures_source ON captures (source);

-- Embeddings live beside the bookmark rather than in it: a bookmark may be
-- re-embedded by a newer model, and keeping the vector separate means that is
-- an insert against a new `model` value, not a destructive update.
CREATE TABLE IF NOT EXISTS embeddings (
    bookmark_id  INTEGER NOT NULL REFERENCES bookmarks (id) ON DELETE CASCADE,
    model        TEXT    NOT NULL,
    dimensions   INTEGER NOT NULL,
    -- Little-endian f32s. Brute-force cosine over a few thousand of these is
    -- about a millisecond, which is well inside the budget an approximate
    -- index would buy back, so there is no ANN structure here.
    vector       BLOB    NOT NULL,
    -- Hash of the exact text embedded, so a title arriving later invalidates
    -- the vector without a full re-embed pass.
    input_hash   TEXT    NOT NULL,
    computed_at  TEXT    NOT NULL,
    PRIMARY KEY (bookmark_id, model)
) STRICT;

-- What enrichment learned about a URL, including that it learned nothing.
--
-- Kept out of `bookmarks` because it is a different kind of fact: a bookmark
-- is what the user saved, this is what the network said about it, and the two
-- have different lifetimes. Recording failure explicitly is what stops the
-- worker from retrying a decade-dead link on every pass.
CREATE TABLE IF NOT EXISTS fetch_state (
    bookmark_id   INTEGER PRIMARY KEY REFERENCES bookmarks (id) ON DELETE CASCADE,
    status        TEXT    NOT NULL CHECK (status IN ('enriched', 'no_metadata', 'dead', 'failed')),
    detail        TEXT,
    attempts      INTEGER NOT NULL DEFAULT 1,
    last_attempt  TEXT    NOT NULL
) STRICT;

CREATE INDEX IF NOT EXISTS fetch_state_status ON fetch_state (status);

-- Bearer credentials for non-browser callers — an agent reaching /v1/tools.
--
-- Only the hash is stored. A leaked database therefore yields no usable
-- credential, and a token that is lost cannot be recovered, only replaced,
-- which is the correct trade for something that grants write access to the
-- archive.
CREATE TABLE IF NOT EXISTS service_tokens (
    id          TEXT PRIMARY KEY,
    -- Operator label, e.g. "odin". Not a secret and not used for lookup.
    name        TEXT NOT NULL,
    token_hash  TEXT NOT NULL UNIQUE,
    -- 'read' or 'read_write'. Fixed for the token's life: widening happens by
    -- minting a new token and revoking the old, so a credential can never gain
    -- authority it was not issued with.
    scope       TEXT NOT NULL CHECK (scope IN ('read', 'read_write')),
    created_at  TEXT NOT NULL,
    -- Set rather than deleted, so a revoked token stays visible in an audit.
    revoked_at  TEXT
) STRICT;

CREATE TABLE IF NOT EXISTS tags (
    id    INTEGER PRIMARY KEY,
    name  TEXT    NOT NULL UNIQUE
) STRICT;

-- `origin` distinguishes a tag the user set from one a model proposed, and
-- `confidence` is only meaningful for the latter. A model proposal never
-- overwrites a human tag; both rows coexist and the serving path prefers the
-- human one.
CREATE TABLE IF NOT EXISTS bookmark_tags (
    bookmark_id  INTEGER NOT NULL REFERENCES bookmarks (id) ON DELETE CASCADE,
    tag_id       INTEGER NOT NULL REFERENCES tags (id) ON DELETE CASCADE,
    origin       TEXT    NOT NULL CHECK (origin IN ('human', 'model')),
    confidence   REAL,
    model        TEXT,
    created_at   TEXT    NOT NULL,
    PRIMARY KEY (bookmark_id, tag_id, origin)
) STRICT;

CREATE INDEX IF NOT EXISTS bookmark_tags_tag ON bookmark_tags (tag_id);

-- Implicit feedback. This is the training signal for ranking and for the
-- revisit-probability model, and it is the one table whose value compounds
-- purely by the system being used.
CREATE TABLE IF NOT EXISTS events (
    id           INTEGER PRIMARY KEY,
    bookmark_id  INTEGER NOT NULL REFERENCES bookmarks (id) ON DELETE CASCADE,
    kind         TEXT    NOT NULL CHECK (kind IN ('opened', 'dismissed', 'searched_click', 'archived')),
    query        TEXT,
    position     INTEGER,
    occurred_at  TEXT    NOT NULL
) STRICT;

CREATE INDEX IF NOT EXISTS events_bookmark ON events (bookmark_id);
CREATE INDEX IF NOT EXISTS events_occurred ON events (occurred_at DESC);

-- Lexical search over the text a bookmark actually has. Kept as an external
-- content table so the bookmark row stays the single source of truth.
CREATE VIRTUAL TABLE IF NOT EXISTS bookmarks_fts USING fts5 (
    canonical_url,
    title,
    description,
    content = 'bookmarks',
    content_rowid = 'id',
    tokenize = 'unicode61'
);

CREATE TRIGGER IF NOT EXISTS bookmarks_fts_insert AFTER INSERT ON bookmarks BEGIN
    INSERT INTO bookmarks_fts (rowid, canonical_url, title, description)
    VALUES (new.id, new.canonical_url, new.title, new.description);
END;

CREATE TRIGGER IF NOT EXISTS bookmarks_fts_delete AFTER DELETE ON bookmarks BEGIN
    INSERT INTO bookmarks_fts (bookmarks_fts, rowid, canonical_url, title, description)
    VALUES ('delete', old.id, old.canonical_url, old.title, old.description);
END;

CREATE TRIGGER IF NOT EXISTS bookmarks_fts_update AFTER UPDATE ON bookmarks BEGIN
    INSERT INTO bookmarks_fts (bookmarks_fts, rowid, canonical_url, title, description)
    VALUES ('delete', old.id, old.canonical_url, old.title, old.description);
    INSERT INTO bookmarks_fts (rowid, canonical_url, title, description)
    VALUES (new.id, new.canonical_url, new.title, new.description);
END;
