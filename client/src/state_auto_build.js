const DEFAULT_SETTINGS = Object.freeze({
  paused: false,
  reserveSteel: 200,
  reserveOil: 100,
});
const RESERVE_MAX = 9_950;

export function autoBuild(value, netStatus = null) {
  return {
    paused: value?.paused === true,
    reserveSteel: boundedReserve(value?.reserveSteel, DEFAULT_SETTINGS.reserveSteel),
    reserveOil: boundedReserve(value?.reserveOil, DEFAULT_SETTINGS.reserveOil),
    ack: Number.isInteger(netStatus?.lastSimConsumedClientSeq)
      ? netStatus.lastSimConsumedClientSeq
      : 0,
  };
}

function boundedReserve(value, fallback) {
  return Number.isInteger(value) && value >= 0 && value <= RESERVE_MAX ? value : fallback;
}
