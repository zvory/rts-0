export function decodeCompactAbilityObjectOwnerState(record, readArray, readU32, readNumber) {
  const fields = readArray(record, "abilityObject.ownerState", 8);
  if (fields.length !== 8) throw new Error("abilityObject.ownerState field count mismatch");
  const state = {};
  if (fields[0] != null) {
    state.earliestReturnTick = readU32(fields[0], "abilityObject.ownerState.earliestReturnTick");
  }
  if (fields[1] != null) state.hp = readU32(fields[1], "abilityObject.ownerState.hp");
  if (fields[2] != null) state.radius = readNumber(fields[2], "abilityObject.ownerState.radius");
  if (fields[3] != null) {
    state.destroyedLockoutTicks = readU32(
      fields[3],
      "abilityObject.ownerState.destroyedLockoutTicks",
    );
  }
  if (fields[4] != null) {
    state.distanceTraveled = readNumber(fields[4], "abilityObject.ownerState.distanceTraveled");
  }
  if (fields[5] != null) state.ticksOut = readU32(fields[5], "abilityObject.ownerState.ticksOut");
  if (fields[6] != null) state.maxHp = readU32(fields[6], "abilityObject.ownerState.maxHp");
  if (fields[7] != null) state.durationTicks = readU32(fields[7], "abilityObject.ownerState.durationTicks");
  return state;
}
