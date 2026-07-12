# Babylon.js runtime note

The opt-in Lab renderer loads `babylonjs` 7.54.3 from jsDelivr only when
`rtsRenderer=babylon` is selected. The package is distributed under the Apache License 2.0;
source and license metadata are available from the pinned npm package. Pixi remains the default
and its startup path does not request this dependency.
