const DECAL_BASE_URL = "/assets/decals";

function asset(id, file, width, height) {
  return Object.freeze({
    id,
    url: `${DECAL_BASE_URL}/${file}`,
    width,
    height,
  });
}

export const INFANTRY_DECAL_ASSETS = Object.freeze([
  asset("rifleman-death-01", "infantry-deaths/rifleman-death-01.png", 224, 160),
  asset("rifleman-death-02", "infantry-deaths/rifleman-death-02.png", 224, 160),
  asset("rifleman-death-03", "infantry-deaths/rifleman-death-03.png", 224, 160),
  asset("rifleman-death-04", "infantry-deaths/rifleman-death-04.png", 224, 160),
  asset("machine-gunner-death-01", "infantry-deaths/machine-gunner-death-01.png", 224, 160),
  asset("machine-gunner-death-02", "infantry-deaths/machine-gunner-death-02.png", 224, 160),
  asset("machine-gunner-death-03", "infantry-deaths/machine-gunner-death-03.png", 224, 160),
  asset("machine-gunner-death-04", "infantry-deaths/machine-gunner-death-04.png", 224, 160),
]);

export const VEHICLE_SCORCH_DECAL_ASSETS = Object.freeze([
  asset("vehicle-scorch-01", "vehicle-scorch-01.svg", 72, 48),
  asset("vehicle-scorch-02", "vehicle-scorch-02.svg", 72, 48),
  asset("vehicle-scorch-03", "vehicle-scorch-03.svg", 72, 48),
  asset("vehicle-scorch-04", "vehicle-scorch-04.svg", 72, 48),
  asset("vehicle-scorch-05", "vehicle-scorch-05.svg", 72, 48),
  asset("vehicle-scorch-06", "vehicle-scorch-06.svg", 72, 48),
  asset("vehicle-scorch-07", "vehicle-scorch-07.svg", 72, 48),
  asset("vehicle-scorch-08", "vehicle-scorch-08.svg", 72, 48),
]);

export const VEHICLE_PAINT_DECAL_ASSETS = Object.freeze([
  asset("vehicle-paint-01", "vehicle-paint-01.svg", 72, 48),
  asset("vehicle-paint-02", "vehicle-paint-02.svg", 72, 48),
  asset("vehicle-paint-03", "vehicle-paint-03.svg", 72, 48),
  asset("vehicle-paint-04", "vehicle-paint-04.svg", 72, 48),
  asset("vehicle-paint-05", "vehicle-paint-05.svg", 72, 48),
  asset("vehicle-paint-06", "vehicle-paint-06.svg", 72, 48),
  asset("vehicle-paint-07", "vehicle-paint-07.svg", 72, 48),
  asset("vehicle-paint-08", "vehicle-paint-08.svg", 72, 48),
]);

export const MORTAR_BLAST_DECAL_ASSETS = Object.freeze([
  asset("mortar-blast-01", "mortar-blast-01.svg", 112, 112),
]);

export const ARTILLERY_BLAST_DECAL_ASSETS = Object.freeze([
  asset("artillery-blast-01", "artillery-blast-01.svg", 216, 216),
]);

export const GROUND_DECAL_ASSET_MANIFEST = Object.freeze({
  infantry: INFANTRY_DECAL_ASSETS,
  vehicleScorch: VEHICLE_SCORCH_DECAL_ASSETS,
  vehiclePaint: VEHICLE_PAINT_DECAL_ASSETS,
  mortarBlast: MORTAR_BLAST_DECAL_ASSETS,
  artilleryBlast: ARTILLERY_BLAST_DECAL_ASSETS,
});
