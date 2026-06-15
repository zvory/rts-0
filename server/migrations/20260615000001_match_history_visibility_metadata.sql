-- Persist enough match metadata to upload replay artifacts for all deployed matches while
-- keeping the public Recent Matches feed limited to human-involved, non-debug games.

alter table matches
    add column if not exists human_count integer not null default 0 check (human_count >= 0),
    add column if not exists debug_mode boolean not null default false;

update matches
set human_count = cardinality(participants)
where human_count = 0;

create index if not exists matches_recent_visible_idx
    on matches (started_at desc)
    where not local_only and human_count >= 1 and not debug_mode;
