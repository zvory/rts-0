import { HudSelectionPanel } from "../../client/src/hud_selection_panel.js";
import { KIND } from "../../client/src/protocol.js";
import { assert } from "./assertions.mjs";
import { withFakeHudDocument } from "./fakes.mjs";

withFakeHudDocument(({ FakeElement }) => {
  const panel = new FakeElement("section");
  const tanks = Array.from({ length: 3 }, (_, index) => ({
    id: 1100 + index,
    owner: 1,
    kind: KIND.TANK,
  }));
  const iconMarkup = '<svg data-test-unit-icon="tank"></svg>';
  const iconOptions = [];
  let teamColor = "#0072b2";
  const selectionPanel = new HudSelectionPanel(
    panel,
    {
      selectedEntities: () => tanks,
      playerById: (id) => id === 1 ? { id: 1, color: teamColor } : null,
    },
    null,
    (kind, options) => {
      iconOptions.push(options);
      return kind === KIND.TANK ? iconMarkup : "";
    },
  );

  selectionPanel.render();
  const blocks = panel.querySelectorAll(".sel-budget-block");
  assert(
    blocks.length === tanks.length &&
      blocks.every((block) =>
        block.className.includes("has-unit-render-icon") && block.innerHTML === iconMarkup),
    "HUD multi-selection blocks render injected renderer-authored unit icons",
  );
  assert(
    iconOptions.every((options) => options?.teamColor === "#0072b2"),
    "HUD multi-selection icons receive their owning player's team color",
  );

  teamColor = "#d55e00";
  selectionPanel.render();
  assert(
    iconOptions.at(-1)?.teamColor === "#d55e00",
    "HUD multi-selection icons refresh when player color metadata changes",
  );
});
