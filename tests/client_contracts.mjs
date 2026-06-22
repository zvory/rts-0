// tests/client_contracts.mjs
// Stable client contract runner. Domain modules carry the assertions so
// future failures can be triaged by contract area without loading one giant file.

import { runFrameProfilerContracts } from "./client_contracts/frame_profiler_contracts.mjs";
import { runScoreboardContracts } from "./client_contracts/scoreboard_contracts.mjs";

await import("./client_contracts/settings_contracts.mjs");
await import("./client_contracts/hud_contracts.mjs");
runFrameProfilerContracts();
await import("./client_contracts/frame_entity_contracts.mjs");
await import("./client_contracts/launch_url_contracts.mjs");
await import("./client_contracts/renderer_contracts.mjs");
await import("./client_contracts/client_boundary_contracts.mjs");
await import("./client_contracts/renderer_feedback_contracts.mjs");
await import("./client_contracts/input_contracts.mjs");
await import("./client_contracts/match_replay_contracts.mjs");
await import("./client_contracts/protocol_contracts.mjs");
await import("./client_contracts/lobby_contracts.mjs");
runScoreboardContracts();
await import("./client_contracts/net_contracts.mjs");
await import("./client_contracts/lab_contracts.mjs");
await import("./client_contracts/command_budget_contracts.mjs");
await import("./client_contracts/prediction_controller_contracts.mjs");
await import("./client_contracts/replay_branch_contracts.mjs");
await import("./client_contracts/config_contracts.mjs");
await import("./client_contracts/state_input_contracts.mjs");
await import("./client_contracts/command_composer_contracts.mjs");
await import("./client_contracts/camera_fog_contracts.mjs");
await import("./client_contracts/audio_contracts.mjs");
await import("./client_contracts/observer_analysis_contracts.mjs");
await import("./client_contracts/map_editor_contracts.mjs");

console.log("✅ client_contracts.mjs: all contract assertions passed");
