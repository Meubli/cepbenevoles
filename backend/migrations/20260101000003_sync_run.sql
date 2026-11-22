-- Sync run history, so the Synchro screen (specs.md §7.3) can show
-- "dernière synchro", "+N depuis", and surface row-level errors instead of
-- them only existing in container logs (technical-specs.md §8).

CREATE TABLE sync_run (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    sheet_id TEXT NOT NULL,
    started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    finished_at TIMESTAMPTZ,
    inserted INT NOT NULL DEFAULT 0,
    already_present INT NOT NULL DEFAULT 0,
    unrouted INT NOT NULL DEFAULT 0,
    skipped_availabilities INT NOT NULL DEFAULT 0,
    -- Set when the whole sheet failed (auth, fetch, header, config) rather
    -- than individual rows — see sync_row_error for per-row problems.
    error TEXT
);

CREATE TABLE sync_row_error (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    sync_run_id UUID NOT NULL REFERENCES sync_run (id) ON DELETE CASCADE,
    sheet_row INT NOT NULL,
    message TEXT NOT NULL
);
