-- Persist replay artifacts separately from match-history score data.
-- `matches.score_screen` stays the score-screen payload; this table stores the deterministic
-- replay command-log artifact for compatible replay launch.

create table if not exists match_replays (
    id                      bigserial primary key,
    match_id                bigint      not null unique references matches(id) on delete cascade,
    artifact_schema_version integer     not null check (artifact_schema_version > 0),
    build_sha               text        not null,
    map_name                text        not null,
    map_schema_version      integer     not null check (map_schema_version > 0),
    map_hash                text        not null,
    duration_ticks          integer     not null check (duration_ticks >= 0),
    artifact_json           jsonb       not null,
    created_at              timestamptz not null default now(),
    updated_at              timestamptz not null default now()
);

create index if not exists match_replays_match_id_idx on match_replays (match_id);
create index if not exists match_replays_build_sha_idx on match_replays (build_sha);
