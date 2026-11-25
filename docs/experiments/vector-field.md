# Magic Dao Vector Field prototype

Current iteration: distinct `vectorField` ability for Magic Adepts and Magic Masters. Ekat keeps
Magic Anchor and its existing radial pull unchanged. The first click fixes the rectangle center;
click-drag or click twice supplies direction. Field width is six tiles perpendicular to the arrow;
depth is three tiles along it. The green arrow/outline has no particles.

Every affected allied or enemy ground unit receives 4 world pixels of added displacement per tick,
regardless of footing or weight. Terrain and hostile-body admission remain. Field lifetime is ten
seconds and cast range is 7.5 tiles. Magic Dao keeps its death Insight bonus inside its new field.
Separate ability and world-object ids prevent interaction with Ekat's Anchor recast/line-shot origin.

This remains a local visual-review prototype. Placement is immediate, not queued; overlapping
Vector Fields currently select the first active field rather than stacking. Compact snapshot v73
carries public field direction under normal fog visibility. Prior weighted/unweighted recordings
are historical prototypes and no longer represent the current balance.

Current gameplay recording source: `media_729f3dccf50cb3f8ad670e847c9f25c8`. Focused checks: 25 Anchor regressions, three Vector Field cases, staged client targeting and separate Adept/Master/Ekat command cards, protocol/catalog parity, client architecture, and strict simulation-library Clippy. Vector Field range/lifetime constants are independent of Anchor.
