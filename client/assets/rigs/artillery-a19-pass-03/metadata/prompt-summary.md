# A-19 modular artillery pass 03

Fresh regeneration of the modular Soviet 122 mm gun M1931/37 (A-19) sheet
using the pass-02 prompt structure. It keeps exactly four disconnected
components: two complete support trails, a central two-wheel carriage, and a
separate barrel/cradle/breech with an oversized recoil assembly.

The pass changes one visual requirement: the complete weapon assembly is
pitched upward out of the ground plane toward the elevated camera. Its muzzle
face is visible, the barrel is foreshortened, and the recoil housing exposes
clear depth while the carriage and trails retain an RTS above-view. This makes
the elevation read as indirect-fire artillery rather than a diagonal flat gun.

Runtime review treatment remains temporary:

- the upper source trail maps to the in-game left trail and receives a fixed
  purple tint;
- both complete trail crop frames carry a black rectangular border;
- the mounting-ring center is the origin for each trail;
- a fixed sheet-space rotation aligns the pitched weapon assembly with the SVG
  rig's authoritative weapon-facing angle;
- the SVG Artillery rig remains authoritative for setup visibility, carriage
  and weapon facing, recoil, muzzle flash, and gameplay anchors.

Generated with the built-in image-generation tool on a removable magenta key,
then converted locally to alpha with soft matte and despill. The exact prompt
is stored in `generation-prompt.txt`.
