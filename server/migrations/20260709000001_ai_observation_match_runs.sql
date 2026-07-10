-- Persist the live correlation id so an AI observation can be retrieved by the same id that
-- appears in its match, performance, and client-network structured log lines.

alter table matches add column if not exists match_run_id text;

create unique index if not exists matches_match_run_id_unique_idx
    on matches (match_run_id)
    where match_run_id is not null;
