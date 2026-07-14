/**
 * Sound manifest. URLs are served by the Rust process from `client/assets`.
 * IDs are stable seams referenced by app, input, HUD, match, and lobby modules.
 */
export const SOUND_MANIFEST = Object.freeze([
  { id: "notice_under_attack", url: "/assets/sound/alert/alert_under_attack_01.mp3", category: "alert" },
  { id: "notice_supply", url: "/assets/sound/alert/alert_supply_low_01.mp3", category: "alert" },
  { id: "notice_steel", url: "/assets/sound/alert/alert_steel_low_01.mp3", category: "alert" },
  { id: "notice_oil", url: "/assets/sound/alert/alert_oil_low_01.mp3", category: "alert" },
  { id: "notice_cannot_build", url: "/assets/sound/alert/alert_cannot_build_01.mp3", category: "alert" },
  { id: "notice_out_of_range", url: "/assets/sound/alert/alert_out_of_range_01.mp3", category: "alert" },
  { id: "build_confirm", url: "/assets/sound/buildings/buildings_construction_start_01.mp3", category: "ui" },
  { id: "combat_tank_01", url: "/assets/sound/combat/combat_tank_cannon_01.mp3", category: "combat_other" },
  { id: "combat_rifle_02", url: "/assets/sound/combat/combat_kar98k_02.mp3", category: "combat_other" },
  { id: "combat_rifle_03", url: "/assets/sound/combat/combat_kar98k_03.mp3", category: "combat_other" },
  { id: "combat_mg_burst_02", url: "/assets/sound/combat/combat_mg42_burst_02.mp3", category: "combat_other" },
  { id: "combat_mg_burst_03", url: "/assets/sound/combat/combat_mg42_burst_03.mp3", category: "combat_other" },
  { id: "combat_panzerfaust_launch_01", url: "/assets/sound/combat/combat_panzerfaust_launch_01.mp3", category: "combat_other" },
  { id: "combat_panzerfaust_impact_01", url: "/assets/sound/combat/combat_panzerfaust_impact_01.mp3", category: "combat_other" },
  { id: "combat_mortar_launch_04", url: "/assets/sound/combat/combat_mortar_launch_04.mp3", category: "combat_other" },
  { id: "combat_artillery_fire_05", url: "/assets/sound/combat/combat_artillery_fire_05.mp3", category: "combat_other" },
  { id: "combat_artillery_landing_01", url: "/assets/sound/combat/combat_artillery_landing_01.mp3", category: "combat_other" },
  { id: "combat_distant_bed_01", url: "/assets/sound/combat/combat_distant_bed_01.mp3", category: "combat_other" },
  { id: "unit_breakthrough_todes_rit_01", url: "/assets/sound/units/units_breakthrough_todes_rit_01.mp3", category: "unit_voice" },
  { id: "unit_breakthrough_koste_es_01", url: "/assets/sound/units/units_breakthrough_koste_es_01.mp3", category: "unit_voice" },
  { id: "countdown_drei", url: "/assets/sound/ui/ui_countdown_drei_01.mp3", category: "ui" },
  { id: "countdown_zwei", url: "/assets/sound/ui/ui_countdown_zwei_01.mp3", category: "ui" },
  { id: "countdown_eins", url: "/assets/sound/ui/ui_countdown_eins_01.mp3", category: "ui" },
  { id: "victory", url: "/assets/sound/ui/ui_victory_01.mp3", category: "ui" },
  { id: "defeat", url: "/assets/sound/ui/ui_defeat_01.mp3", category: "ui" },
]);
