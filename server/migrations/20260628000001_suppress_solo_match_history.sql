-- Treat one-human, no-AI sandbox matches as debug sessions. Keep their rows and replay
-- artifacts for diagnostics, but suppress them from the Recent Matches feed.

update matches
set debug_mode = true
where not debug_mode
  and human_count = 1
  and cardinality(participants) = 1;

drop index if exists matches_recent_visible_idx;

create index if not exists matches_recent_visible_idx
    on matches (started_at desc)
    where not local_only
      and human_count >= 1
      and not debug_mode
      and not (human_count = 1 and cardinality(participants) = 1);
