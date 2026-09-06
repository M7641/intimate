CREATE TABLE IF NOT EXISTS challenges (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL,
    signature   TEXT NOT NULL,
    created_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS test_cases (
    id           TEXT PRIMARY KEY,
    challenge_id TEXT NOT NULL REFERENCES challenges(id),
    input        TEXT NOT NULL,
    expected     TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS candidates (
    id           TEXT PRIMARY KEY,
    challenge_id TEXT NOT NULL REFERENCES challenges(id),
    generation   INTEGER NOT NULL,
    strategy     TEXT NOT NULL,
    source_code  TEXT NOT NULL,
    created_at   TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS results (
    id              TEXT PRIMARY KEY,
    candidate_id    TEXT NOT NULL REFERENCES candidates(id),
    passed          INTEGER NOT NULL DEFAULT 0,
    score           REAL NOT NULL DEFAULT 0.0,
    time_ns         INTEGER NOT NULL DEFAULT 0,
    memory_bytes    INTEGER NOT NULL DEFAULT 0,
    binary_size     INTEGER NOT NULL DEFAULT 0,
    compile_time_ns INTEGER NOT NULL DEFAULT 0,
    error           TEXT
);

CREATE INDEX IF NOT EXISTS idx_candidates_challenge_gen ON candidates(challenge_id, generation);
CREATE INDEX IF NOT EXISTS idx_results_candidate ON results(candidate_id);
