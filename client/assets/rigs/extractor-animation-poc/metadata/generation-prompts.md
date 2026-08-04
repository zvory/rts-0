# Extractor animation POC generation prompts

Built-in ImageGen was used for both source sheets. Each was generated on a flat magenta chroma-key
background, converted to alpha, cropped into isolated components, and packed into 128 px frames.

## Steel Mine

Low-detail PS1-era RTS source components included a compact steel ore chunk, a separate brown-handled
steel pickaxe, and an angular spark burst. The shipped POC atlas intentionally retains only the
pickaxe; the existing world Steel resource supplies the target and no impact spark is rendered.

## Pump Jack

The replacement Pump Jack is a straight-on front elevation with a camera raised about 30 degrees and
zero left-right yaw. It uses one symmetrical dark A-frame with a centered pivot and one separate
horizontal ivory walking beam with a short polished rod. The prompt explicitly prohibited diagonal,
three-quarter, and isometric views, along with ground slabs, concrete pads, pipes, tanks, text,
shadows, and scenery. Components are isolated on uniform `#ff00ff` with generous padding.
