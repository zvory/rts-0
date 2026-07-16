use axum::extract::Query;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Redirect};

use rts_server::dev_scenarios::{
    all_dev_scenarios, dev_scenario_blocker_label, dev_scenario_case_label,
    dev_scenario_unit_label, parse_dev_scenario_launch_with_case,
};

pub async fn dev_scenario_handler(
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let id = params.get("id").map(|s| s.trim()).unwrap_or("");
    let unit = params.get("unit").map(|s| s.trim()).unwrap_or("");
    let count = params.get("count").map(|s| s.trim()).unwrap_or("");
    let blocker = params
        .get("blocker")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    let case = params
        .get("case")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    if id.is_empty() && unit.is_empty() && count.is_empty() && case.is_none() {
        return Html(dev_scenario_index_html()).into_response();
    }
    if let Some(launch) = parse_dev_scenario_launch_with_case(id, unit, count, blocker, case) {
        let mut target = format!(
            "/?watchScenario=1&id={}&unit={}&count={}",
            launch.id, launch.unit, launch.count
        );
        if let Some(blocker) = launch.blocker {
            target.push_str("&blocker=");
            target.push_str(blocker.stable_id());
        } else if launch.id == "vehicle_small_block_baseline" {
            target.push_str("&blocker=none");
        }
        if let Some(case) = launch.case {
            target.push_str("&case=");
            target.push_str(case);
        }
        return Redirect::temporary(&target).into_response();
    }
    (
        StatusCode::BAD_REQUEST,
        "supported dev scenario urls are listed at /dev/scenarios",
    )
        .into_response()
}

fn dev_scenario_index_html() -> String {
    let mut items = String::new();
    for scenario in all_dev_scenarios() {
        if scenario.id == "tank_trap_pathing_matrix" {
            items.push_str(&dev_scenario_case_matrix_html(
                scenario.title,
                scenario.id,
                scenario.description,
                scenario.launches,
            ));
            continue;
        }
        let mut counts = Vec::new();
        let mut rows_by_variant = Vec::new();
        for launch in scenario.launches {
            if !counts.contains(&launch.count) {
                counts.push(launch.count);
            }
            let variant = (launch.unit, launch.blocker);
            if !rows_by_variant.contains(&variant) {
                rows_by_variant.push(variant);
            }
        }
        counts.sort_unstable();

        let mut header_cells = String::new();
        for count in &counts {
            header_cells.push_str(&format!("<th scope=\"col\">x{count}</th>"));
        }

        let mut rows = String::new();
        for (unit, blocker) in rows_by_variant {
            let mut cells = String::new();
            for count in &counts {
                if scenario.launches.iter().any(|candidate| {
                    candidate.unit == unit
                        && candidate.count == *count
                        && candidate.blocker == blocker
                }) {
                    let blocker_query = match blocker {
                        Some(kind) => format!("&blocker={}", kind.stable_id()),
                        None if scenario.id == "vehicle_small_block_baseline" => {
                            "&blocker=none".to_string()
                        }
                        None => String::new(),
                    };
                    cells.push_str(&format!(
                        "<td><a class=\"scenario-link\" href=\"/dev/scenarios?id={}&unit={}&count={}{}\">Open</a></td>",
                        scenario.id,
                        unit,
                        count,
                        blocker_query
                    ));
                } else {
                    cells.push_str("<td class=\"scenario-missing\">-</td>");
                }
            }
            let row_label = if scenario.id == "vehicle_small_block_baseline" {
                format!(
                    "{} / blocker: {}",
                    dev_scenario_unit_label(unit),
                    dev_scenario_blocker_label(blocker)
                )
            } else {
                dev_scenario_unit_label(unit).to_string()
            };
            rows.push_str(&format!(
                "<tr>\
                    <th scope=\"row\">{}</th>\
                    {}\
                 </tr>",
                row_label, cells
            ));
        }

        items.push_str(&format!(
            "<section class=\"scenario-panel\">\
                <div class=\"scenario-copy\">\
                  <h2>{}</h2>\
                  <p><code>{}</code></p>\
                  <p>{}</p>\
                </div>\
                <table class=\"scenario-table\">\
                  <thead>\
                    <tr>\
                      <th scope=\"col\">Unit</th>\
                      {}\
                    </tr>\
                  </thead>\
                  <tbody>{}</tbody>\
                </table>\
             </section>",
            scenario.title, scenario.id, scenario.description, header_cells, rows
        ));
    }

    format!(
        "<!DOCTYPE html>\
        <html lang=\"en\">\
          <head>\
            <meta charset=\"UTF-8\" />\
            <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\" />\
            <title>Dev Scenarios</title>\
            <link rel=\"stylesheet\" href=\"/styles.css\" />\
            <style>\
              html, body {{ min-height: 100%; height: auto; overflow-y: auto; overflow-x: hidden; }}\
              body {{ background: var(--void); color: var(--ink); font-family: var(--font); image-rendering: auto; }}\
              .scenario-page {{ width: min(960px, 100%); margin: 0 auto; padding: 32px 20px 48px; }}\
              .scenario-page h1 {{ margin: 0 0 6px; color: var(--accent); font-size: 28px; line-height: 1.1; letter-spacing: 0.08em; text-transform: uppercase; text-shadow: 2px 2px 0 #191710; }}\
              .scenario-page > p {{ margin: 0; color: var(--ink-dim); }}\
              .scenario-grid {{ display: grid; gap: 14px; margin-top: 22px; }}\
              .scenario-panel {{ display: grid; grid-template-columns: minmax(220px, 0.8fr) minmax(0, 1.2fr); gap: 18px; align-items: start; border: 1px solid var(--panel-edge); border-radius: var(--radius); background: rgba(39, 37, 31, 0.98); box-shadow: var(--shadow), inset 0 1px 0 rgba(255, 255, 255, 0.08); padding: 18px; }}\
              .scenario-copy h2 {{ margin: 0 0 8px; font-size: 16px; letter-spacing: 0.04em; text-transform: uppercase; }}\
              .scenario-copy p {{ margin: 0 0 8px; color: var(--ink-dim); }}\
              .scenario-copy code {{ color: var(--ink-faint); font-family: var(--mono); font-size: 12px; }}\
              .scenario-table {{ width: 100%; border-collapse: collapse; border: 1px solid rgba(91, 83, 65, 0.7); background: rgba(17, 17, 15, 0.34); }}\
              .scenario-table th, .scenario-table td {{ padding: 9px 10px; border-bottom: 1px solid rgba(91, 83, 65, 0.45); text-align: left; }}\
              .scenario-table thead th {{ color: var(--ink-dim); font-family: var(--mono); font-size: 11px; font-weight: 700; letter-spacing: 0.08em; text-transform: uppercase; }}\
              .scenario-table tbody th {{ color: var(--ink); font-weight: 600; }}\
              .scenario-table tbody tr:last-child th, .scenario-table tbody tr:last-child td {{ border-bottom: 0; }}\
              .scenario-link {{ display: inline-flex; align-items: center; justify-content: center; min-width: 56px; padding: 6px 10px; border: 1px solid var(--panel-edge); border-radius: var(--radius-sm); background: var(--panel); color: var(--ink); font-weight: 600; text-decoration: none; }}\
              .scenario-link:hover {{ border-color: var(--accent); box-shadow: inset 0 0 0 1px var(--panel-glow); }}\
              .scenario-form {{ display: grid; gap: 10px; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr) auto; align-items: end; }}\
              .scenario-field {{ display: grid; gap: 5px; }}\
              .scenario-field span {{ color: var(--ink-dim); font-family: var(--mono); font-size: 11px; font-weight: 700; letter-spacing: 0.08em; text-transform: uppercase; }}\
              .scenario-field select {{ width: 100%; min-height: 34px; border: 1px solid rgba(91, 83, 65, 0.8); border-radius: var(--radius-sm); background: rgba(17, 17, 15, 0.86); color: var(--ink); padding: 6px 8px; font: inherit; }}\
              .scenario-missing {{ color: var(--ink-faint); font-family: var(--mono); }}\
              @media (max-width: 720px) {{ .scenario-panel {{ grid-template-columns: 1fr; }} .scenario-page {{ padding: 24px 12px 36px; }} .scenario-table th, .scenario-table td {{ padding: 8px; }} .scenario-form {{ grid-template-columns: 1fr; }} }}\
            </style>\
          </head>\
          <body>\
            <main class=\"scenario-page\">\
              <h1>Dev Scenarios</h1>\
              <p>Available local scenario launches. Pick one to open the live no-fog watcher.</p>\
              <div class=\"scenario-grid\">{items}</div>\
            </main>\
          </body>\
        </html>"
    )
}

fn dev_scenario_case_matrix_html(
    title: &str,
    id: &str,
    description: &str,
    launches: &[rts_server::dev_scenarios::DevScenarioLaunch],
) -> String {
    let first = launches
        .first()
        .expect("case-matrix scenario should have at least one launch");
    let mut cases = Vec::new();
    let mut unit_options = String::new();
    for launch in launches {
        let Some(case) = launch.case else {
            continue;
        };
        if !cases.contains(&case) {
            cases.push(case);
        }
        unit_options.push_str(&format!(
            "<option value=\"{}\" data-case=\"{}\">{}</option>",
            launch.unit,
            case,
            dev_scenario_unit_label(launch.unit)
        ));
    }

    let mut case_options = String::new();
    for case in cases {
        case_options.push_str(&format!(
            "<option value=\"{}\">{}</option>",
            case,
            dev_scenario_case_label(case)
        ));
    }

    let default_case = first.case.unwrap_or("");
    format!(
        "<section class=\"scenario-panel\">\
            <div class=\"scenario-copy\">\
              <h2>{title}</h2>\
              <p><code>{id}</code></p>\
              <p>{description}</p>\
            </div>\
            <form class=\"scenario-form\" action=\"/dev/scenarios\" method=\"get\" data-case-matrix>\
              <input type=\"hidden\" name=\"id\" value=\"{id}\" />\
              <input type=\"hidden\" name=\"count\" value=\"1\" />\
              <label class=\"scenario-field\">\
                <span>Case</span>\
                <select name=\"case\" data-case-select>{case_options}</select>\
              </label>\
              <label class=\"scenario-field\">\
                <span>Unit</span>\
                <select name=\"unit\" data-unit-select data-default-case=\"{default_case}\">{unit_options}</select>\
              </label>\
              <button class=\"scenario-link\" type=\"submit\">Open</button>\
            </form>\
            <script>\
              (() => {{\
                const form = document.currentScript.previousElementSibling;\
                const caseSelect = form.querySelector('[data-case-select]');\
                const unitSelect = form.querySelector('[data-unit-select]');\
                const refresh = () => {{\
                  const selected = caseSelect.value;\
                  let firstVisible = null;\
                  for (const option of unitSelect.options) {{\
                    const visible = option.dataset.case === selected;\
                    option.hidden = !visible;\
                    option.disabled = !visible;\
                    if (visible && !firstVisible) firstVisible = option;\
                  }}\
                  if (unitSelect.selectedOptions.length === 0 || unitSelect.selectedOptions[0].disabled) {{\
                    unitSelect.selectedIndex = firstVisible ? firstVisible.index : -1;\
                  }}\
                }};\
                caseSelect.addEventListener('change', refresh);\
                refresh();\
              }})();\
            </script>\
         </section>"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenario_index_lists_supported_launches() {
        let html = dev_scenario_index_html();
        assert!(html.contains("Scout Car Snaking Corridor"));
        assert!(html.contains("Direct Reverse Order"));
        assert!(html.contains("Replay 112 Vehicle Lock"));
        assert!(html.contains("Vehicle Wall Chokepoint"));
        assert!(html.contains("Vehicle Corner Wall"));
        assert!(html.contains("Vehicle Small-Unit Block Baseline"));
        assert!(html.contains("<table class=\"scenario-table\">"));
        assert!(html.contains("/dev/scenarios?id=scout_car_snaking_corridor&unit=worker&count=1"));
        assert!(html.contains("/dev/scenarios?id=scout_car_snaking_corridor&unit=tank&count=4"));
        assert!(html.contains("/dev/scenarios?id=direct_reverse_order&unit=tank&count=1"));
        assert!(html.contains("/dev/scenarios?id=replay_142_vehicle_lock&unit=scout_car&count=2"));
        assert!(
            html.contains("/dev/scenarios?id=scout_car_wall_chokepoint&unit=scout_car&count=15")
        );
        assert!(html.contains("/dev/scenarios?id=scout_car_wall_chokepoint&unit=tank&count=15"));
        assert!(html
            .contains("/dev/scenarios?id=scout_car_wall_chokepoint&unit=anti_tank_gun&count=15"));
        assert!(html.contains("/dev/scenarios?id=vehicle_corner_wall&unit=anti_tank_gun&count=5"));
        assert!(html.contains("/dev/scenarios?id=vehicle_corner_wall&unit=scout_car&count=5"));
        assert!(html.contains("/dev/scenarios?id=vehicle_corner_wall&unit=tank&count=5"));
        assert!(
            html.contains("/dev/scenarios?id=vehicle_small_block_baseline&unit=scout_car&count=5")
        );
        assert!(html.contains(
            "/dev/scenarios?id=vehicle_small_block_baseline&unit=scout_car&count=5&blocker=none"
        ));
        assert!(html.contains(
            "/dev/scenarios?id=vehicle_small_block_baseline&unit=scout_car&count=5&blocker=machine_gunner"
        ));
        assert!(html.contains(
            "/dev/scenarios?id=vehicle_small_block_baseline&unit=tank&count=5&blocker=anti_tank_gun"
        ));
        assert!(html.contains("/dev/scenarios?id=factory_zero_gap_perpendicular&unit=tank&count=1"));
        assert!(html.contains("Command Car Building Corner"));
        assert!(
            html.contains("/dev/scenarios?id=command_car_building_corner&unit=command_car&count=1")
        );
        assert!(html.contains("Command Car Building Corner — West-Southwest"));
        assert!(html.contains(
            "/dev/scenarios?id=command_car_building_corner_west_southwest&unit=command_car&count=1"
        ));
        assert!(html.contains("Tank Trap Pathing Matrix"));
        assert!(html.contains("<select name=\"case\""));
        assert!(html.contains("<option value=\"friendly_vehicle_reroute\""));
        assert!(html.contains("<option value=\"enemy_vehicle_breach\""));
        assert!(html.contains("<option value=\"infantry_pass_through\""));
        assert!(html.contains("<option value=\"explicit_infantry_attack\""));
        assert!(
            html.contains("<input type=\"hidden\" name=\"id\" value=\"tank_trap_pathing_matrix\"")
        );
        assert!(html.contains("Entrenchment Inspection"));
        assert!(html.contains("/dev/scenarios?id=entrenchment_inspection&unit=rifleman&count=1"));
        assert!(html.contains("/dev/scenarios?id=tank_coax_inspection&unit=tank&count=1"));
    }
}
