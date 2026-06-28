-- Match history: allow deploy-drain finalization to distinguish aborted rows from draws.

alter table matches
    drop constraint if exists matches_outcome_check;

alter table matches
    add constraint matches_outcome_check
    check (outcome in ('win', 'draw', 'aborted'));
