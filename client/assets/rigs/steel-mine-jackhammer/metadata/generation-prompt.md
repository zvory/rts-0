# Steel Mine jackhammer generation

The selected source was generated with built-in ImageGen as an oversized late-1930s/1940s
pneumatic paving breaker: a cast-metal cylindrical body, bolted collars, twin loop handles, dark
grips, and a long steel chisel. Broad painted surfaces were constrained to neutral matte white and
light cool gray so the runtime `team-light` multiply tint can apply owner color while preserving
dark mechanical contrast.

The source used a uniform magenta chroma background with no scenery, shadow, dust, debris, sparks,
text, or operator. The chroma background was removed with the shared ImageGen alpha helper, then
the selected tool was trimmed and centered into the 128 px runtime frame.

The animation is code-driven. A seven-stroke-per-second vertical piston motion carries nearly all
of the displacement. A slow sub-pixel horizontal drift moves consecutive impacts slightly around
the contact point without rotating, scaling, or rapidly shaking the housing.
