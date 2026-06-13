use rts_rules::{defs, economy, faction, EntityKind};

fn main() {
    let catalog = faction::CURRENT_CATALOG;
    println!("{{");
    println!("  \"id\": \"{}\",", catalog.id);
    print_kind_array("units", catalog.units, true);
    print_kind_array("buildings", catalog.buildings, true);
    println!("  \"buildables\": [");
    for (index, kind) in catalog.buildables.iter().enumerate() {
        let comma = if index + 1 == catalog.buildables.len() {
            ""
        } else {
            ","
        };
        print!("    {{\"kind\":\"{}\",\"requires\":", kind.stable_id());
        let requires = defs::building_def(*kind)
            .map(|def| def.build_requires)
            .unwrap_or(&[]);
        print_kind_array_inline(requires);
        println!("}}{comma}");
    }
    println!("  ],");
    println!("  \"trainables\": [");
    let producers: Vec<_> = catalog
        .buildings
        .iter()
        .copied()
        .filter(|kind| !catalog.trainable_units(*kind).is_empty())
        .collect();
    for (index, building) in producers.iter().enumerate() {
        let comma = if index + 1 == producers.len() {
            ""
        } else {
            ","
        };
        print!("    {{\"building\":\"{}\",\"units\":", building.stable_id());
        print_kind_vec_inline(&catalog.trainable_units(*building));
        println!("}}{comma}");
    }
    println!("  ],");
    println!("  \"research\": [");
    for (index, upgrade) in catalog.upgrades.iter().enumerate() {
        let comma = if index + 1 == catalog.upgrades.len() {
            ""
        } else {
            ","
        };
        println!(
            "    {{\"id\":\"{}\",\"researchedAt\":\"{}\"}}{comma}",
            upgrade.id,
            upgrade.researched_at.stable_id()
        );
    }
    println!("  ],");
    println!("  \"abilities\": [");
    for (index, ability) in catalog.abilities.iter().enumerate() {
        let comma = if index + 1 == catalog.abilities.len() {
            ""
        } else {
            ","
        };
        print!("    {{\"id\":\"{}\",\"carriers\":", ability.id);
        print_kind_array_inline(ability.carriers);
        println!("}}{comma}");
    }
    println!("  ],");
    println!("  \"costs\": {{");
    let priced: Vec<_> = catalog
        .units
        .iter()
        .chain(catalog.buildings.iter())
        .copied()
        .collect();
    for (index, kind) in priced.iter().enumerate() {
        let comma = if index + 1 == priced.len() { "" } else { "," };
        let (steel, oil) = economy::cost(*kind);
        println!(
            "    \"{}\": {{\"steel\":{},\"oil\":{}}}{comma}",
            kind.stable_id(),
            steel,
            oil
        );
    }
    println!("  }}");
    println!("}}");
}

fn print_kind_array(name: &str, kinds: &[EntityKind], comma: bool) {
    print!("  \"{name}\": ");
    print_kind_array_inline(kinds);
    println!("{}", if comma { "," } else { "" });
}

fn print_kind_array_inline(kinds: &[EntityKind]) {
    print!("[");
    for (index, kind) in kinds.iter().enumerate() {
        if index > 0 {
            print!(",");
        }
        print!("\"{}\"", kind.stable_id());
    }
    print!("]");
}

fn print_kind_vec_inline(kinds: &[EntityKind]) {
    print_kind_array_inline(kinds);
}
