-- Tag developer-local match-history rows so production/beta recent matches can hide them.

alter table matches
    add column if not exists local_only boolean not null default false;

create index if not exists matches_started_at_public_idx
    on matches (started_at desc)
    where not local_only;
