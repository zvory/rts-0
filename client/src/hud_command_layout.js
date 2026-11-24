// Existing command slot ids stay stable as the card gains a fourth column.
//   Q W E R
//   A S D F
//   Z X C V
export const GRID_HOTKEYS = Object.freeze(["Q", "W", "E", "A", "S", "D", "Z", "X", "C"]);

// The fourth column holds movement (R), defense (F), and attack (V) items.
export const COMMAND_CARD_SLOT_ORDER = Object.freeze([0, 1, 2, 9, 3, 4, 5, 10, 6, 7, 8, 11]);
const COMMAND_CARD_HOTKEYS = Object.freeze([...GRID_HOTKEYS, "R", "F", "V"]);

export function gridHotkeyForSlot(slotIndex) {
  return COMMAND_CARD_HOTKEYS[slotIndex] || "";
}
