use rts_rules::{defs, economy, faction, EntityKind};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let catalogs: Vec<_> = if args.iter().any(|arg| arg == "--all") {
        faction::CATALOGS.to_vec()
    } else if let Some(index) = args.iter().position(|arg| arg == "--faction") {
        let Some(id) = args.get(index + 1) else {
            eprintln!("--faction requires an id");
            std::process::exit(2);
        };
        let Some(catalog) = faction::catalog_for(id) else {
            eprintln!("unknown faction catalog: {id}");
            std::process::exit(1);
        };
        vec![catalog]
    } else {
        vec![faction::CURRENT_CATALOG]
    };

    if catalogs.len() == 1 && !args.iter().any(|arg| arg == "--all") {
        print_catalog(catalogs[0], "");
        println!();
    } else {
        println!("{{");
        println!("  \"catalogs\": [");
        for (index, catalog) in catalogs.iter().enumerate() {
            print_catalog(*catalog, "    ");
            println!("{}", if index + 1 == catalogs.len() { "" } else { "," });
        }
        println!("  ]");
        println!("}}");
    }
}

fn print_catalog(catalog: faction::FactionCatalog, indent: &str) {
    println!("{indent}{{");
    println!("{indent}  \"id\": \"{}\",", catalog.id);
    println!("{indent}  \"loadoutId\": \"{}\",", catalog.loadout.id);
    print_kind_array("units", catalog.units, true, indent);
    print_kind_array("buildings", catalog.buildings, true, indent);
    println!("{indent}  \"buildables\": [");
    for (index, kind) in catalog.buildables.iter().enumerate() {
        let comma = if index + 1 == catalog.buildables.len() {
            ""
        } else {
            ","
        };
        print!(
            "{indent}    {{\"kind\":\"{}\",\"requires\":",
            kind.stable_id()
        );
        let requires = defs::building_def(*kind)
            .map(|def| def.build_requires)
            .unwrap_or(&[]);
        print_kind_array_inline(requires);
        println!("}}{comma}");
    }
    println!("{indent}  ],");
    println!("{indent}  \"trainables\": [");
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
        print!(
            "{indent}    {{\"building\":\"{}\",\"units\":",
            building.stable_id()
        );
        print_kind_vec_inline(&catalog.trainable_units(*building));
        println!("}}{comma}");
    }
    println!("{indent}  ],");
    println!("{indent}  \"research\": [");
    for (index, upgrade) in catalog.upgrades.iter().enumerate() {
        let comma = if index + 1 == catalog.upgrades.len() {
            ""
        } else {
            ","
        };
        println!(
            "{indent}    {{\"id\":\"{}\",\"researchedAt\":\"{}\"}}{comma}",
            upgrade.id,
            upgrade.researched_at.stable_id()
        );
    }
    println!("{indent}  ],");
    println!("{indent}  \"abilities\": [");
    for (index, ability) in catalog.abilities.iter().enumerate() {
        let comma = if index + 1 == catalog.abilities.len() {
            ""
        } else {
            ","
        };
        print!("{indent}    {{\"id\":\"{}\",\"carriers\":", ability.id);
        print_kind_array_inline(ability.carriers);
        println!("}}{comma}");
    }
    println!("{indent}  ],");
    println!("{indent}  \"costs\": {{");
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
            "{indent}    \"{}\": {{\"steel\":{},\"oil\":{}}}{comma}",
            kind.stable_id(),
            steel,
            oil
        );
    }
    println!("{indent}  }}");
    print!("{indent}}}");
}

fn print_kind_array(name: &str, kinds: &[EntityKind], comma: bool, indent: &str) {
    print!("{indent}  \"{name}\": ");
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
