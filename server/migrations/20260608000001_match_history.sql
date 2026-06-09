-- Match history: one row per resolved multiplayer match.
-- Written by the authoritative server in `end_match`. Untrusted clients never write here.

create table if not exists matches (
    id              bigserial primary key,
    started_at      timestamptz not null,
    ended_at        timestamptz not null default now(),
    duration_ms     integer     not null check (duration_ms >= 0),
    map_name        text        not null,
    winner_name     text,
    outcome         text        not null check (outcome in ('win', 'draw')),
    participants    text[]      not null,
    score_screen    jsonb       not null
);

create index if not exists matches_started_at_idx on matches (started_at desc);
create index if not exists matches_map_name_idx   on matches (map_name);
