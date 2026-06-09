-- Hide rows written by automated smoke/integration tests or bot matches before the recording
-- eligibility checks existed. Keep them as local-only history instead of deleting data.

update matches
set local_only = true
where not local_only
  and (
    exists (
      select 1
      from unnest(participants) as participant(name)
      where participant.name like 'Computer %'
         or lower(participant.name) = 'smoke'
    )
    or participants @> array['Alpha', 'Bravo']::text[]
  );
