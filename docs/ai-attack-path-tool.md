# AI attack-path analysis tool

`ai-map-analysis-debug` can render likely enemy approaches to one or more defended bases. It uses
the same authored map loader and static passability analysis as the live AI and writes an SVG map
plus a machine-readable JSON report.

```powershell
cargo run --manifest-path server/Cargo.toml -p rts-ai --bin ai-map-analysis-debug -- `
  --map "Schone Tage" --players 2 --defender 1 --bases 2 --paths-per-base 3 `
  --layers attack-paths,chokes,bases --out D:\rts-ai-results\schone-tage-defense.svg
```

The JSON is written beside the SVG with the same filename stem. Red is the primary shortest
formation-safe route, orange and yellow are progressively more costly alternate or flanking
routes, purple diamonds are route bottlenecks/openings, cyan squares are suggested static
interception points, and blue squares are defended bases.

## Base selection

One-base mode defends the selected player's authored start. Additional bases are selected from
reachable resource-cluster sites. Enemy home sites are excluded.

Expansion selection is intentionally not nearest-first. Candidate sites inside a plausible
expansion-distance envelope are ranked by:

1. fewest attack corridors not shared with the home defense;
2. fewest approaches that avoid a detected choke;
3. greatest route overlap with existing home-defense lanes;
4. narrowest worst bottleneck; and
5. distance from home.

This lets a slightly farther, safer expansion beat a close site that creates new flanks, while
preventing a remote site behind the home base from winning solely because all attacks pass through
the home defense.

## Limits

This is static strategic analysis. It knows authored terrain, starts, and resource sites, but not
future player-built walls, temporary unit congestion, vision, or actual enemy intent. Treat the
routes as defensive coverage lanes. Live reactions should later combine these lanes with observed
enemy positions and the locations of buildings Jeff has actually constructed.
