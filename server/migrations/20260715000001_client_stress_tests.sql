create table if not exists client_stress_tests (
    run_id text primary key,
    artifact_label text not null,
    received_at timestamptz not null default now(),
    build_id text not null,
    status text not null check (status in ('completed', 'invalid')),
    user_label text not null default '',
    device_id text not null,
    fingerprint text not null,
    platform text not null default '',
    average_fps_x100 integer not null default 0,
    frame_work_p95_ms integer not null default 0,
    renderer_p95_ms integer not null default 0,
    profile_kind text not null,
    profile_sample_count integer not null default 0,
    artifact_json jsonb not null
);

create index if not exists client_stress_tests_received_at_idx
    on client_stress_tests (received_at desc);

create index if not exists client_stress_tests_fingerprint_idx
    on client_stress_tests (fingerprint, received_at desc);
