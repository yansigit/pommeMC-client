//! Generates pomme's per-version block data from Mojang's data-generator
//! reports (`java -DbundlerMainClass=net.minecraft.data.Main -jar server.jar
//! --reports`).
//!
//! Usage:
//!   blockgen blocks <reports/blocks.json> <version> <out.json>
//!   blockgen behavior <azalea generated.rs> <out.json>
//!   blockgen sounds <Blocks.java> <SoundType.java> <out.json>
//!   blockgen state <generated/state.json> <blocks-<v>.json> <out.json>
//!
//! `blocks` flattens the report into a compact per-block table (name, first
//! state id, default state id, ordered property lists). Every explicit state
//! id + property set in the report is cross-checked against the cartesian
//! reconstruction the client uses, and the id space is verified dense — the
//! tool hard-fails rather than emit silently-wrong data.
//!
//! `behavior` seeds the name-keyed destroy-time table from an azalea
//! `generated.rs` (e.g. `~/.cargo/git/checkouts/azalea-*/<rev>/azalea-block/
//! src/generated.rs`); new blocks the seed doesn't know must be appended by
//! hand from the decompiled `Blocks.java`. Hand-added entries survive a
//! regen (existing keys the seed doesn't produce are carried over and
//! listed), but the seed wins for keys it does produce — a hand-correction
//! to a seeded value does not survive.
//!
//! `sounds` extracts each explicit `.sound(SoundType.X)` assignment from
//! decompiled `Blocks.java`, resolves copied block properties, and reads the
//! corresponding hit/break events, volume, and pitch from `SoundType.java`.
//! Blocks constructed through helpers that do not expose their sound type are
//! carried from the existing output, and are listed so a new unresolved block
//! cannot silently enter the table.
//!
//! `state` compacts the raw per-state property dump produced by running
//! vanilla (`tools/stategen/StateDump.java`, see `just stategen`) into the
//! per-block table the client embeds: each field is a scalar when uniform
//! across the block's states, else a per-state array, and face-occlusion
//! masks are deduped into a dictionary. State counts and value ranges are
//! cross-checked against the version's blocks table.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.as_slice() {
        [cmd, report, version, out] if cmd == "blocks" => gen_blocks(report, version, out),
        [cmd, generated, out] if cmd == "behavior" => gen_behavior(generated, out),
        [cmd, blocks, sound_types, out] if cmd == "sounds" => {
            gen_sounds(blocks, sound_types, out)
        }
        [cmd, dump, blocks, out] if cmd == "state" => gen_state(dump, blocks, out),
        _ => Err("usage: blockgen blocks <blocks.json> <version> <out.json>\n       blockgen behavior <generated.rs> <out.json>\n       blockgen sounds <Blocks.java> <SoundType.java> <out.json>\n       blockgen state <state.json> <blocks-<v>.json> <out.json>".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("blockgen: {e}");
            ExitCode::FAILURE
        }
    }
}

type Error = Box<dyn std::error::Error>;

struct Block {
    name: String,
    first_id: u32,
    default_id: u32,
    /// Property (key, values) pairs in the report's listed order; the last
    /// property varies fastest in the state-id cartesian product.
    props: Vec<(String, Vec<String>)>,
}

fn gen_blocks(report_path: &str, version: &str, out_path: &str) -> Result<(), Error> {
    let report: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(report_path)?)?;

    let mut blocks = Vec::new();
    for (name, entry) in &report {
        blocks.push(parse_block(name, entry)?);
    }
    blocks.sort_by_key(|b| b.first_id);

    // The id space must tile densely from 0 with no gaps or overlaps.
    let mut expected_id = 0u32;
    for block in &blocks {
        if block.first_id != expected_id {
            return Err(format!(
                "id space not dense: block '{}' starts at {} but expected {}",
                block.name, block.first_id, expected_id
            )
            .into());
        }
        expected_id += state_count(block);
    }

    let mut out = String::new();
    writeln!(out, "{{")?;
    writeln!(out, "  \"version\": {},", serde_json::to_string(version)?)?;
    writeln!(out, "  \"state_count\": {expected_id},")?;
    writeln!(out, "  \"blocks\": [")?;
    for (i, block) in blocks.iter().enumerate() {
        let comma = if i + 1 < blocks.len() { "," } else { "" };
        let mut line = format!(
            "    {{\"name\": {}, \"first_id\": {}, \"default_id\": {}",
            serde_json::to_string(&block.name)?,
            block.first_id,
            block.default_id
        );
        if !block.props.is_empty() {
            let props: Vec<serde_json::Value> = block
                .props
                .iter()
                .map(|(k, vs)| serde_json::json!([k, vs]))
                .collect();
            write!(line, ", \"props\": {}", serde_json::to_string(&props)?)?;
        }
        writeln!(out, "{line}}}{comma}")?;
    }
    writeln!(out, "  ]")?;
    writeln!(out, "}}")?;

    std::fs::write(out_path, &out)?;
    println!(
        "wrote {} blocks / {} states for {} to {}",
        blocks.len(),
        expected_id,
        version,
        out_path
    );
    Ok(())
}

fn state_count(block: &Block) -> u32 {
    block.props.iter().map(|(_, vs)| vs.len() as u32).product()
}

fn parse_block(name: &str, entry: &serde_json::Value) -> Result<Block, Error> {
    let name = name.strip_prefix("minecraft:").unwrap_or(name).to_string();

    // Value arrays are in variant order, but the report's property KEY order
    // is the builder order, not the state-enumeration order (vanilla sorts
    // properties by name for the state definition). Rather than assume the
    // sort rule, each property's stride is derived from the explicit state
    // ids below and the properties reordered to match.
    let mut props: Vec<(String, Vec<String>)> = Vec::new();
    if let Some(properties) = entry.get("properties") {
        let map = properties
            .as_object()
            .ok_or_else(|| format!("{name}: properties is not an object"))?;
        for (key, values) in map {
            let values = values
                .as_array()
                .ok_or_else(|| format!("{name}: property {key} values not an array"))?
                .iter()
                .map(|v| {
                    v.as_str()
                        .map(String::from)
                        .ok_or_else(|| format!("{name}: property {key} has non-string value"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            props.push((key.clone(), values));
        }
    }

    let states = entry
        .get("states")
        .and_then(|s| s.as_array())
        .ok_or_else(|| format!("{name}: missing states array"))?;

    let expected: u32 = props.iter().map(|(_, vs)| vs.len() as u32).product();
    if states.len() as u32 != expected {
        return Err(format!(
            "{name}: {} states but property product is {expected}",
            states.len()
        )
        .into());
    }

    let first_id = states
        .iter()
        .filter_map(|s| s.get("id").and_then(|i| i.as_u64()))
        .min()
        .ok_or_else(|| format!("{name}: states missing ids"))? as u32;

    // Index the explicit states by their full property assignment.
    let mut by_props: std::collections::HashMap<BTreeMap<&str, &str>, u32> =
        std::collections::HashMap::new();
    for state in states {
        let id = state
            .get("id")
            .and_then(|i| i.as_u64())
            .ok_or_else(|| format!("{name}: state missing id"))? as u32;
        let mut key = BTreeMap::new();
        if let Some(map) = state.get("properties").and_then(|p| p.as_object()) {
            for (k, v) in map {
                key.insert(
                    k.as_str(),
                    v.as_str()
                        .ok_or_else(|| format!("{name}: non-string property value"))?,
                );
            }
        }
        by_props.insert(key, id);
    }

    // The base state (first id) must sit at every property's first value,
    // and flipping one property to its second value reveals that property's
    // stride in the enumeration.
    let base: BTreeMap<&str, &str> = props
        .iter()
        .map(|(k, vs)| (k.as_str(), vs[0].as_str()))
        .collect();
    if by_props.get(&base) != Some(&first_id) {
        return Err(format!(
            "{name}: base state (all first values) is not the first id — enumeration isn't a plain cartesian product"
        )
        .into());
    }
    let mut strides: Vec<u32> = Vec::with_capacity(props.len());
    for (key, values) in &props {
        if values.len() == 1 {
            strides.push(0);
            continue;
        }
        let mut flipped = base.clone();
        flipped.insert(key.as_str(), values[1].as_str());
        let id = by_props
            .get(&flipped)
            .ok_or_else(|| format!("{name}: no state for {key}={}", values[1]))?;
        strides.push(id - first_id);
    }

    // Reorder to enumeration order: largest stride first (single-value
    // properties contribute factor 1 and can go last).
    let mut order: Vec<usize> = (0..props.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(strides[i]));
    let props: Vec<(String, Vec<String>)> = order.into_iter().map(|i| props[i].clone()).collect();

    let mut default_id = None;

    // Cross-check every explicit state against the cartesian reconstruction
    // (derived property order, last property varying fastest).
    for state in states {
        let id = state
            .get("id")
            .and_then(|i| i.as_u64())
            .ok_or_else(|| format!("{name}: state missing id"))? as u32;
        let offset = id
            .checked_sub(first_id)
            .ok_or_else(|| format!("{name}: state id {id} below first id {first_id}"))?;

        let mut stride: u32 = props.iter().map(|(_, vs)| vs.len() as u32).product();
        for (key, values) in &props {
            stride /= values.len() as u32;
            let index = (offset / stride) as usize % values.len();
            let reconstructed = &values[index];
            let reported = state
                .get("properties")
                .and_then(|p| p.get(key))
                .and_then(|v| v.as_str())
                .ok_or_else(|| format!("{name}: state {id} missing property {key}"))?;
            if reconstructed != reported {
                return Err(format!(
                    "{name}: state {id} property {key} reconstructs to '{reconstructed}' but report says '{reported}' — enumeration order changed, extend the format"
                )
                .into());
            }
        }

        if state.get("default").and_then(|d| d.as_bool()) == Some(true) {
            default_id = Some(id);
        }
    }

    let default_id = default_id.ok_or_else(|| format!("{name}: no default state"))?;
    Ok(Block {
        name,
        first_id,
        default_id,
        props,
    })
}

/// Extracts `destroy_time` + `requires_correct_tool_for_drops` per block from
/// azalea's machine-generated `generated.rs` block list (uniform shape:
/// `name => BlockBehavior::new().strength(a, b)..., {`).
fn gen_behavior(generated_path: &str, out_path: &str) -> Result<(), Error> {
    let source = std::fs::read_to_string(generated_path)?;
    let mut entries: BTreeMap<String, (f32, bool)> = BTreeMap::new();

    for line in source.lines() {
        let trimmed = line.trim_start();
        let Some((name, rest)) = trimmed.split_once(" => BlockBehavior::new()") else {
            continue;
        };
        if !name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            continue;
        }
        let destroy_time = extract_float_arg(rest, ".strength(")
            .or_else(|| extract_float_arg(rest, ".destroy_time("))
            .unwrap_or(0.0);
        let requires_tool = rest.contains(".requires_correct_tool_for_drops()");
        entries.insert(name.to_string(), (destroy_time, requires_tool));
    }

    if entries.is_empty() {
        return Err("no block behavior entries found — wrong input file?".into());
    }

    // Carry over hand-appended keys the seed doesn't know.
    let mut carried: Vec<String> = Vec::new();
    match std::fs::read_to_string(out_path) {
        Ok(existing) => {
            let existing: BTreeMap<String, BehaviorJson> =
                serde_json::from_str(&existing).map_err(|e| format!("{out_path}: {e}"))?;
            for (name, b) in existing {
                if !entries.contains_key(&name) {
                    entries.insert(name.clone(), (b.destroy_time, b.requires_correct_tool));
                    carried.push(name);
                }
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("{out_path}: {e}").into()),
    }

    let mut out = String::new();
    writeln!(out, "{{")?;
    let len = entries.len();
    for (i, (name, (destroy_time, requires_tool))) in entries.iter().enumerate() {
        let comma = if i + 1 < len { "," } else { "" };
        writeln!(
            out,
            "  {}: {{\"destroy_time\": {destroy_time}, \"requires_correct_tool\": {requires_tool}}}{comma}",
            serde_json::to_string(name)?
        )?;
    }
    writeln!(out, "}}")?;

    std::fs::write(out_path, &out)?;
    println!("wrote {len} behavior entries to {out_path}");
    if !carried.is_empty() {
        println!(
            "carried over {} hand-added entries: {}",
            carried.len(),
            carried.join(", ")
        );
    }
    Ok(())
}

/// An entry of the emitted `block_behavior.json`, read back on regen.
#[derive(serde::Deserialize)]
struct BehaviorJson {
    destroy_time: f32,
    requires_correct_tool: bool,
}

type SoundJson = (String, String, f32, f32);

#[derive(Debug)]
struct BlockSoundSource {
    constant: String,
    name: String,
    registry_alias: Option<String>,
    sound_type: Option<String>,
    copied_from: Option<String>,
    uses_default_properties: bool,
}

fn gen_sounds(blocks_path: &str, sound_types_path: &str, out_path: &str) -> Result<(), Error> {
    let blocks_source = std::fs::read_to_string(blocks_path)?;
    let sound_types_source = std::fs::read_to_string(sound_types_path)?;
    let sound_types = parse_sound_types(&sound_types_source)?;
    let mut blocks = parse_block_sound_sources(&blocks_source)?;

    let existing: BTreeMap<String, SoundJson> = match std::fs::read_to_string(out_path) {
        Ok(source) => serde_json::from_str(&source).map_err(|e| format!("{out_path}: {e}"))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
        Err(e) => return Err(format!("{out_path}: {e}").into()),
    };
    for block in &mut blocks {
        if !existing.contains_key(&block.name)
            && let Some(alias) = &block.registry_alias
            && existing.contains_key(alias)
        {
            block.name = alias.clone();
        }
    }

    let stone = sound_types
        .get("STONE")
        .ok_or("SoundType.java has no STONE declaration")?
        .clone();
    let mut entries = BTreeMap::new();
    let mut unresolved = Vec::new();

    // Direct assignments and vanilla's bare `Properties.of()` default are
    // authoritative immediately. Resolve property copies in a second pass so
    // declaration order is irrelevant.
    for block in &blocks {
        if let Some(sound_type) = &block.sound_type {
            let sounds = sound_types
                .get(sound_type)
                .ok_or_else(|| format!("{}: unknown SoundType.{sound_type}", block.name))?;
            entries.insert(block.name.clone(), sounds.clone());
        } else if block.uses_default_properties {
            entries.insert(block.name.clone(), stone.clone());
        }
    }
    loop {
        let before = entries.len();
        for block in &blocks {
            if entries.contains_key(&block.name) {
                continue;
            }
            if let Some(parent) = &block.copied_from
                && let Some(sounds) = entries.get(parent).or_else(|| existing.get(parent))
            {
                entries.insert(block.name.clone(), sounds.clone());
            }
        }
        if entries.len() == before {
            break;
        }
    }

    for block in &blocks {
        if entries.contains_key(&block.name) {
            continue;
        }
        if let Some(sounds) = existing.get(&block.name) {
            entries.insert(block.name.clone(), sounds.clone());
            unresolved.push(block.name.clone());
        } else {
            return Err(format!(
                "{}: sound type is hidden behind an unsupported helper and no existing entry can seed it",
                block.name
            )
            .into());
        }
    }

    let parsed_names: std::collections::HashSet<&str> =
        blocks.iter().map(|block| block.name.as_str()).collect();
    let mut carried = Vec::new();
    for (name, sounds) in existing {
        if !parsed_names.contains(name.as_str()) {
            entries.insert(name.clone(), sounds);
            carried.push(name);
        }
    }

    let mut out = String::new();
    writeln!(out, "{{")?;
    let len = entries.len();
    for (index, (name, sounds)) in entries.iter().enumerate() {
        let comma = if index + 1 < len { "," } else { "" };
        writeln!(
            out,
            "  {}: {}{comma}",
            serde_json::to_string(name)?,
            serde_json::to_string(sounds)?
        )?;
    }
    writeln!(out, "}}")?;
    std::fs::write(out_path, out)?;

    println!(
        "wrote {len} block sound entries from {} SoundTypes to {out_path}",
        sound_types.len()
    );
    if !unresolved.is_empty() {
        println!(
            "retained {} helper-defined entries from the existing table: {}",
            unresolved.len(),
            unresolved.join(", ")
        );
    }
    if !carried.is_empty() {
        println!(
            "carried {} entries absent from Blocks.java: {}",
            carried.len(),
            carried.join(", ")
        );
    }
    Ok(())
}

fn parse_sound_types(source: &str) -> Result<BTreeMap<String, SoundJson>, Error> {
    let mut sounds = BTreeMap::new();
    for statement in java_declarations(source, "public static final SoundType ")? {
        let name = declaration_name(statement, "public static final SoundType ")?;
        let args = constructor_args(statement, "new SoundType(")?;
        if args.len() != 7 {
            return Err(format!(
                "SoundType.{name}: expected 7 constructor arguments, got {}",
                args.len()
            )
            .into());
        }
        let volume = parse_java_float(args[0])?;
        let pitch = parse_java_float(args[1])?;
        let break_event = sound_event_id(args[2])?;
        let hit_event = sound_event_id(args[5])?;
        sounds.insert(name.to_string(), (hit_event, break_event, volume, pitch));
    }
    if sounds.is_empty() {
        return Err("no SoundType declarations found — wrong input file?".into());
    }
    Ok(sounds)
}

fn parse_block_sound_sources(source: &str) -> Result<Vec<BlockSoundSource>, Error> {
    let mut blocks = Vec::new();
    for statement in java_declarations(source, "public static final Block ")? {
        let constant = declaration_name(statement, "public static final Block ")?;
        let registry_alias = ["BlockIds.", "BlockItemIds."]
            .into_iter()
            .find_map(|prefix| extract_java_constant(statement, prefix))
            .map(str::to_ascii_lowercase)
            .filter(|alias| alias != &constant.to_ascii_lowercase());
        let name = constant.to_ascii_lowercase();
        let sound_type = extract_call_arg(statement, ".sound(SoundType.", ')')
            .map(str::trim)
            .map(str::to_string);
        let copied_from = [
            ".ofLegacyCopy(",
            ".ofFullCopy(",
            ".ofFullCopyShared(",
            ".ofCopy(",
        ]
        .into_iter()
        .find_map(|prefix| extract_call_arg(statement, prefix, ')'))
        .and_then(java_block_constant);
        let uses_default_properties = sound_type.is_none()
            && copied_from.is_none()
            && statement.contains("BlockBehaviour.Properties.of()")
            && !statement.contains("BlockSetType.")
            && !statement.contains("WoodType.");
        blocks.push(BlockSoundSource {
            constant: constant.to_string(),
            name,
            registry_alias,
            sound_type,
            copied_from,
            uses_default_properties,
        });
    }
    if blocks.is_empty() {
        return Err("no Block declarations found — wrong input file?".into());
    }
    let registry_names: std::collections::HashMap<String, String> = blocks
        .iter()
        .map(|block| (block.constant.clone(), block.name.clone()))
        .collect();
    for block in &mut blocks {
        if let Some(parent) = &block.copied_from
            && let Some(name) = registry_names.get(parent)
        {
            block.copied_from = Some(name.clone());
        }
    }
    Ok(blocks)
}

fn java_declarations<'a>(source: &'a str, prefix: &str) -> Result<Vec<&'a str>, Error> {
    let mut declarations = Vec::new();
    let mut offset = 0;
    while let Some(relative) = source[offset..].find(prefix) {
        let start = offset + relative;
        let mut depth = 0i32;
        let mut in_string = false;
        let mut escaped = false;
        let mut end = None;
        for (relative_index, ch) in source[start..].char_indices() {
            if in_string {
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    in_string = false;
                }
                continue;
            }
            match ch {
                '"' => in_string = true,
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' => depth -= 1,
                ';' if depth == 0 => {
                    end = Some(start + relative_index + 1);
                    break;
                }
                _ => {}
            }
        }
        let end = end.ok_or_else(|| format!("unterminated Java declaration after byte {start}"))?;
        declarations.push(&source[start..end]);
        offset = end;
    }
    Ok(declarations)
}

fn declaration_name<'a>(statement: &'a str, prefix: &str) -> Result<&'a str, Error> {
    let rest = statement
        .strip_prefix(prefix)
        .ok_or_else(|| format!("declaration does not start with '{prefix}'"))?;
    rest.split(|ch: char| ch.is_ascii_whitespace() || ch == '=')
        .next()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| "Java declaration has no name".into())
}

fn constructor_args<'a>(statement: &'a str, prefix: &str) -> Result<Vec<&'a str>, Error> {
    let start = statement
        .find(prefix)
        .ok_or_else(|| format!("declaration has no '{prefix}'"))?
        + prefix.len();
    let end = matching_paren_end(statement, start)?;
    Ok(split_java_args(&statement[start..end]))
}

fn matching_paren_end(text: &str, start: usize) -> Result<usize, Error> {
    let mut depth = 0i32;
    for (relative, ch) in text[start..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' if depth == 0 => return Ok(start + relative),
            ')' => depth -= 1,
            _ => {}
        }
    }
    Err("unterminated Java argument list".into())
}

fn split_java_args(text: &str) -> Vec<&str> {
    let mut args = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for (index, ch) in text.char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                args.push(text[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
    }
    args.push(text[start..].trim());
    args
}

fn extract_call_arg<'a>(text: &'a str, prefix: &str, end: char) -> Option<&'a str> {
    let start = text.find(prefix)? + prefix.len();
    let rest = &text[start..];
    Some(&rest[..rest.find(end)?])
}

fn java_block_constant(text: &str) -> Option<String> {
    let value = text.trim().trim_start_matches("Blocks.");
    value
        .chars()
        .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
        .then(|| value.to_string())
}

fn extract_java_constant<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let start = text.find(prefix)? + prefix.len();
    let rest = &text[start..];
    let len = rest
        .bytes()
        .take_while(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || *byte == b'_')
        .count();
    (len > 0).then(|| &rest[..len])
}

fn parse_java_float(text: &str) -> Result<f32, Error> {
    text.trim()
        .trim_end_matches(['F', 'f'])
        .parse()
        .map_err(|e| format!("invalid Java float '{}': {e}", text.trim()).into())
}

fn sound_event_id(text: &str) -> Result<String, Error> {
    let constant = text
        .trim()
        .strip_prefix("SoundEvents.")
        .ok_or_else(|| format!("expected SoundEvents constant, got '{}'", text.trim()))?;
    if constant == "EMPTY" {
        return Ok(String::new());
    }
    let constant = constant.strip_prefix("BLOCK_").unwrap_or(constant);
    let (kind, event) = constant
        .rsplit_once('_')
        .ok_or_else(|| format!("SoundEvents.{constant} has no event suffix"))?;
    Ok(format!(
        "block.{}.{}",
        kind.to_ascii_lowercase(),
        event.to_ascii_lowercase()
    ))
}

fn extract_float_arg(text: &str, method: &str) -> Option<f32> {
    let start = text.find(method)? + method.len();
    let rest = &text[start..];
    let end = rest.find([',', ')'])?;
    rest[..end].trim().parse().ok()
}

/// Raw per-state dump written by `tools/stategen/StateDump.java`.
#[derive(serde::Deserialize)]
struct StateDumpFile {
    version: String,
    state_count: u32,
    emission: Vec<u8>,
    dampening: Vec<u8>,
    propagates_skylight_down: Vec<u8>,
    can_occlude: Vec<u8>,
    use_shape_for_light_occlusion: Vec<u8>,
    /// `BlockBehaviour.hasCollision`; block-level, so uniform across each
    /// block's states.
    has_collision: Vec<u8>,
    /// State id (as string) -> 6 face masks, 64 hex chars each, present
    /// exactly for states with `can_occlude && use_shape_for_light_occlusion`.
    face_masks: std::collections::HashMap<String, [String; 6]>,
}

#[derive(serde::Deserialize)]
struct BlocksFile {
    version: String,
    state_count: u32,
    blocks: Vec<BlocksEntry>,
}

#[derive(serde::Deserialize)]
struct BlocksEntry {
    name: String,
    first_id: u32,
    #[serde(default)]
    props: Vec<(String, Vec<String>)>,
}

fn gen_state(dump_path: &str, blocks_path: &str, out_path: &str) -> Result<(), Error> {
    let dump: StateDumpFile = serde_json::from_str(&std::fs::read_to_string(dump_path)?)?;
    let blocks: BlocksFile = serde_json::from_str(&std::fs::read_to_string(blocks_path)?)?;

    if dump.version != blocks.version {
        return Err(format!(
            "version mismatch: state dump is '{}', blocks table is '{}'",
            dump.version, blocks.version
        )
        .into());
    }
    if dump.state_count != blocks.state_count {
        return Err(format!(
            "state count mismatch: state dump has {}, blocks table has {}",
            dump.state_count, blocks.state_count
        )
        .into());
    }
    let n = dump.state_count as usize;
    for (key, len) in [
        ("emission", dump.emission.len()),
        ("dampening", dump.dampening.len()),
        (
            "propagates_skylight_down",
            dump.propagates_skylight_down.len(),
        ),
        ("can_occlude", dump.can_occlude.len()),
        (
            "use_shape_for_light_occlusion",
            dump.use_shape_for_light_occlusion.len(),
        ),
        ("has_collision", dump.has_collision.len()),
    ] {
        if len != n {
            return Err(format!("{key} has {len} entries, expected {n}").into());
        }
    }
    for i in 0..n {
        if dump.emission[i] > 15 || dump.dampening[i] > 15 {
            return Err(format!("state {i}: light value out of 0..=15 range").into());
        }
        for (key, v) in [
            ("propagates_skylight_down", dump.propagates_skylight_down[i]),
            ("can_occlude", dump.can_occlude[i]),
            (
                "use_shape_for_light_occlusion",
                dump.use_shape_for_light_occlusion[i],
            ),
            ("has_collision", dump.has_collision[i]),
        ] {
            if v > 1 {
                return Err(format!("state {i}: {key} is {v}, expected 0/1").into());
            }
        }
    }

    // Dedupe face masks into a dictionary, iterating in ascending state-id
    // order so the output is deterministic.
    let mut masks_by_state: BTreeMap<u32, &[String; 6]> = BTreeMap::new();
    for (key, masks) in &dump.face_masks {
        let id: u32 = key
            .parse()
            .map_err(|_| format!("face_masks key '{key}' is not a state id"))?;
        if id as usize >= n {
            return Err(format!("face_masks state id {id} out of range").into());
        }
        for mask in masks {
            if mask.len() != 64 || !mask.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(format!("state {id}: face mask '{mask}' is not 64 hex chars").into());
            }
        }
        masks_by_state.insert(id, masks);
    }
    for i in 0..n {
        let shaped = dump.can_occlude[i] == 1 && dump.use_shape_for_light_occlusion[i] == 1;
        if shaped != masks_by_state.contains_key(&(i as u32)) {
            return Err(format!(
                "state {i}: face masks {} but can_occlude && use_shape is {shaped}",
                if shaped { "missing" } else { "present" }
            )
            .into());
        }
    }
    let mut dict: Vec<&str> = Vec::new();
    let mut dict_index: std::collections::HashMap<&str, u32> = std::collections::HashMap::new();
    let mut state_masks: Vec<Option<[u32; 6]>> = vec![None; n];
    for (&id, masks) in &masks_by_state {
        let mut indices = [0u32; 6];
        for (slot, mask) in indices.iter_mut().zip(masks.iter()) {
            *slot = *dict_index.entry(mask).or_insert_with(|| {
                dict.push(mask);
                dict.len() as u32 - 1
            });
        }
        state_masks[id as usize] = Some(indices);
    }

    let mut out = String::new();
    writeln!(out, "{{")?;
    writeln!(
        out,
        "  \"version\": {},",
        serde_json::to_string(&dump.version)?
    )?;
    writeln!(out, "  \"state_count\": {n},")?;
    writeln!(out, "  \"masks\": {},", serde_json::to_string(&dict)?)?;
    writeln!(out, "  \"blocks\": [")?;
    let mut expected_id = 0u32;
    for (i, block) in blocks.blocks.iter().enumerate() {
        if block.first_id != expected_id {
            return Err(format!(
                "blocks table not dense at '{}': starts at {} expected {expected_id}",
                block.name, block.first_id
            )
            .into());
        }
        let count: u32 = block.props.iter().map(|(_, vs)| vs.len() as u32).product();
        let range = block.first_id as usize..(block.first_id + count) as usize;
        expected_id += count;

        let mut line = format!("    {{\"name\": {}", serde_json::to_string(&block.name)?);
        for (key, values) in [
            ("e", &dump.emission[range.clone()]),
            ("d", &dump.dampening[range.clone()]),
            ("p", &dump.propagates_skylight_down[range.clone()]),
            ("o", &dump.can_occlude[range.clone()]),
            ("u", &dump.use_shape_for_light_occlusion[range.clone()]),
        ] {
            write!(line, ", \"{key}\": {}", scalar_or_array(values)?)?;
        }
        let collision = &dump.has_collision[range.clone()];
        if collision.iter().any(|&c| c != collision[0]) {
            return Err(format!("{}: has_collision varies across states", block.name).into());
        }
        write!(line, ", \"c\": {}", collision[0])?;
        let masks = &state_masks[range];
        if masks.iter().any(Option::is_some) {
            if masks.iter().all(|m| *m == masks[0]) {
                // Uniform across the block: a single 6-index tuple.
                write!(
                    line,
                    ", \"f\": {}",
                    serde_json::to_string(&masks[0].unwrap())?
                )?;
            } else {
                // Per state: 6-index tuple or null.
                write!(line, ", \"f\": {}", serde_json::to_string(masks)?)?;
            }
        }
        let comma = if i + 1 < blocks.blocks.len() { "," } else { "" };
        writeln!(out, "{line}}}{comma}")?;
    }
    if expected_id as usize != n {
        return Err(format!("blocks cover {expected_id} states, dump has {n}").into());
    }
    writeln!(out, "  ]")?;
    writeln!(out, "}}")?;

    std::fs::write(out_path, &out)?;
    println!(
        "wrote state data for {} states ({} shaped, {} distinct masks) to {out_path}",
        n,
        masks_by_state.len(),
        dict.len()
    );
    Ok(())
}

/// A single JSON value when every entry is equal, else the full array.
fn scalar_or_array(values: &[u8]) -> Result<String, Error> {
    let first = *values.first().ok_or("block with zero states")?;
    if values.iter().all(|&v| v == first) {
        Ok(first.to_string())
    } else {
        Ok(serde_json::to_string(values)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOUND_TYPES: &str = r#"
        public static final SoundType EMPTY = new SoundType(
            1.0F, 1.0F, SoundEvents.EMPTY, SoundEvents.EMPTY,
            SoundEvents.EMPTY, SoundEvents.EMPTY, SoundEvents.EMPTY
        );
        public static final SoundType STONE = new SoundType(
            1.0F, 1.0F, SoundEvents.STONE_BREAK, SoundEvents.STONE_STEP,
            SoundEvents.STONE_PLACE, SoundEvents.STONE_HIT, SoundEvents.STONE_FALL
        );
        public static final SoundType WOOD = new SoundType(
            0.8F, 1.2F, SoundEvents.WOOD_BREAK, SoundEvents.WOOD_STEP,
            SoundEvents.WOOD_PLACE, SoundEvents.WOOD_HIT, SoundEvents.WOOD_FALL
        );
        public static final SoundType POT = new SoundType(
            1.0F, 1.0F, SoundEvents.DECORATED_POT_SHATTER, SoundEvents.STONE_STEP,
            SoundEvents.STONE_PLACE, SoundEvents.STONE_HIT, SoundEvents.STONE_FALL
        );
    "#;

    #[test]
    fn parses_sound_type_events_volume_pitch_and_special_event_names() {
        let sounds = parse_sound_types(SOUND_TYPES).unwrap();
        assert_eq!(
            sounds.get("WOOD"),
            Some(&(
                "block.wood.hit".to_string(),
                "block.wood.break".to_string(),
                0.8,
                1.2,
            ))
        );
        assert_eq!(sounds.get("POT").unwrap().1, "block.decorated_pot.shatter");
        assert_eq!(sounds.get("EMPTY").unwrap().0, "");
        assert_eq!(sounds.get("EMPTY").unwrap().1, "");
    }

    #[test]
    fn parses_direct_default_copied_and_registry_alias_blocks() {
        let blocks = parse_block_sound_sources(
            r#"
            public static final Block STONE = register(
                BlockItemIds.STONE, BlockBehaviour.Properties.of());
            public static final Block OAK_PLANKS = register(
                BlockItemIds.OAK_PLANKS,
                BlockBehaviour.Properties.of().sound(SoundType.WOOD));
            public static final Block OAK_STAIRS = register(
                BlockItemIds.OAK_STAIRS,
                BlockBehaviour.Properties.ofLegacyCopy(OAK_PLANKS));
            public static final Block POTTED_AZALEA = register(
                BlockIds.POTTED_AZALEA_BUSH,
                p -> new FlowerPotBlock(AZALEA, p), flowerPotProperties());
            public static final Block OAK_DOOR = register(
                BlockItemIds.OAK_DOOR,
                p -> new DoorBlock(BlockSetType.OAK, p),
                BlockBehaviour.Properties.of());
            "#,
        )
        .unwrap();

        assert_eq!(blocks[0].name, "stone");
        assert!(blocks[0].uses_default_properties);
        assert_eq!(blocks[1].sound_type.as_deref(), Some("WOOD"));
        assert_eq!(blocks[2].copied_from.as_deref(), Some("oak_planks"));
        assert_eq!(
            blocks[3].registry_alias.as_deref(),
            Some("potted_azalea_bush")
        );
        assert!(!blocks[4].uses_default_properties);
    }

    #[test]
    fn rejects_unknown_sound_event_shapes() {
        let error = sound_event_id("SoundEvents.BROKEN").unwrap_err();
        assert!(error.to_string().contains("no event suffix"));
    }
}
