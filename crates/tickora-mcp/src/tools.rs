use std::path::Path;

use serde::Deserialize;
use serde_json::{Map, Value, json};
use tickora_core::ecs::{Entity, FieldValue};
use tickora_render::{DEFAULT_SIZE, MAX_SIZE, MIN_SIZE};

use crate::host::Host;
use crate::jsonrpc::RpcError;
use crate::{base64, scenario, shot};

pub const MAX_STEP_TICKS: u64 = 100_000;
pub const DEFAULT_QUERY_LIMIT: u64 = 100;

type Args = Map<String, Value>;

pub fn definitions<H: Host>(host: &H) -> Value {
    let entity_ref = json!({ "type": "integer", "minimum": 0, "maximum": u32::MAX });
    let seed = json!({
        "type": ["integer", "string"],
        "description": "Unsigned 64-bit seed, as an integer or a decimal string for values above 2^53.",
    });
    json!([
        tool(
            "tickora_step",
            "Advance the simulation. The inputs are applied on the first stepped tick only, then the remaining ticks run with no input. Returns the tick and the hashes after the last tick.",
            json!({
                "ticks": { "type": "integer", "minimum": 1, "maximum": MAX_STEP_TICKS, "default": 1 },
                "inputs": { "type": "array", "items": host.input_schema(), "default": [] },
            }),
            &[],
        ),
        tool(
            "tickora_state_hash",
            "Read the current tick, seed, state hash and chained replay hash. Hashes and the seed are decimal strings of unsigned 64-bit values.",
            json!({}),
            &[],
        ),
        tool(
            "tickora_list_components",
            "List the registered component names in registration order, with how many entities hold each.",
            json!({}),
            &[],
        ),
        tool(
            "tickora_query",
            "List live entities that hold every named component, in deterministic store order. An empty list of names matches every live entity.",
            json!({
                "components": { "type": "array", "items": { "type": "string" } },
                "limit": { "type": "integer", "minimum": 0, "default": DEFAULT_QUERY_LIMIT },
            }),
            &["components"],
        ),
        tool(
            "tickora_get_entity",
            "Read every component of one live entity. Fixed-point fields are shown as approximate numbers for reading only.",
            json!({ "index": entity_ref, "generation": entity_ref }),
            &["index", "generation"],
        ),
        tool(
            "tickora_reset",
            "Restart the simulation at tick 0 with a seed, discarding all state and recorded inputs. Without a seed the current one is reused.",
            json!({ "seed": seed }),
            &[],
        ),
        tool(
            "tickora_run_scenario",
            "Run a scenario on a fresh simulation, leaving the live one untouched, and report every assertion. Give exactly one of path (a JSON scenario file, relative to the server's working directory) or scenario (the same object inline). A scenario has seed, optional inputs as [{at, inputs}] applied in the step from tick at to at + 1, run_to, and expect: a list of {at} plus one check, which is state_hash or replay_hash (decimal string), entities (count), component with count, query (component names) with count, or entity {index, generation} with field \"component.field\" and value. Counts and int values take {eq} or {min, max}; bool and text take {eq}; fx takes {raw} (Q32.32 decimal string), {raw_min, raw_max} or {approx, tol}. A failed assertion returns passed false, not a tool error.",
            json!({
                "path": { "type": "string" },
                "scenario": { "type": "object" },
            }),
            &[],
        ),
        tool(
            "tickora_screenshot",
            "Render the live simulation offscreen and return it as a PNG image plus metadata, without stepping it. The camera follows Unreal's viewport model: viewport \"top\" (orthographic, +x right, +y up), location [x, y] at the image centre and ortho_width in world units across the image. Every result carries a camera string such as \"top 0 0 256\"; pass it back as camera to reproduce the view, the way Unreal's BugItGo does. view_mode \"unlit\" fills each entity, \"wireframe\" outlines it. pixel_hash is a hash of the raw RGBA pixels. With path, the PNG is also written there, relative to the server's working directory.",
            json!({
                "viewport": { "type": "string", "enum": ["top"], "default": "top" },
                "location": { "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2, "default": [0, 0] },
                "ortho_width": { "type": "number", "exclusiveMinimum": 0, "default": 256 },
                "camera": { "type": "string", "description": "A camera string from an earlier result, instead of viewport, location and ortho_width." },
                "width": { "type": "integer", "minimum": MIN_SIZE, "maximum": MAX_SIZE, "default": DEFAULT_SIZE },
                "height": { "type": "integer", "minimum": MIN_SIZE, "maximum": MAX_SIZE, "default": DEFAULT_SIZE },
                "view_mode": { "type": "string", "enum": ["unlit", "wireframe"], "default": "unlit" },
                "show_grid": { "type": "boolean", "default": true },
                "path": { "type": "string" },
            }),
            &[],
        ),
    ])
}

pub fn call<H: Host>(host: &mut H, params: &Map<String, Value>) -> Result<Value, RpcError> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| RpcError::invalid_params("tools/call needs a string name"))?;
    let empty = Map::new();
    let args = match params.get("arguments") {
        None | Some(Value::Null) => &empty,
        Some(Value::Object(args)) => args,
        Some(_) => return Err(RpcError::invalid_params("arguments must be an object")),
    };
    let plain = |structured: Value| (structured, None);
    let outcome = match name {
        "tickora_step" => step(host, args).map(plain),
        "tickora_state_hash" => only(args, &[]).map(|()| plain(hashes(host))),
        "tickora_list_components" => only(args, &[]).map(|()| plain(list_components(host))),
        "tickora_query" => query(host, args).map(plain),
        "tickora_get_entity" => get_entity(host, args).map(plain),
        "tickora_reset" => reset(host, args).map(plain),
        "tickora_run_scenario" => run_scenario(host, args).map(plain),
        "tickora_screenshot" => screenshot(host, args),
        _ => return Err(RpcError::invalid_params(format!("unknown tool {name}"))),
    };
    Ok(match outcome {
        Ok((structured, image)) => {
            let mut content: Vec<Value> = image.into_iter().collect();
            content.push(json!({ "type": "text", "text": structured.to_string() }));
            json!({
                "content": content,
                "structuredContent": structured,
                "isError": false,
            })
        }
        Err(message) => json!({
            "content": [{ "type": "text", "text": message }],
            "isError": true,
        }),
    })
}

fn tool(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": {
            "type": "object",
            "properties": properties,
            "required": required,
            "additionalProperties": false,
        },
    })
}

fn only(args: &Args, allowed: &[&str]) -> Result<(), String> {
    match args.keys().find(|key| !allowed.contains(&key.as_str())) {
        Some(key) => Err(format!("unexpected argument {key:?}")),
        None => Ok(()),
    }
}

fn typed<T: for<'de> Deserialize<'de>>(args: &Args, key: &str) -> Result<Option<T>, String> {
    args.get(key)
        .map(|value| T::deserialize(value).map_err(|e| format!("argument {key:?}: {e}")))
        .transpose()
}

fn required<T: for<'de> Deserialize<'de>>(args: &Args, key: &str) -> Result<T, String> {
    typed(args, key)?.ok_or_else(|| format!("missing argument {key:?}"))
}

fn hashes<H: Host>(host: &H) -> Value {
    json!({
        "tick": host.tick(),
        "seed": host.seed().to_string(),
        "state_hash": host.state_hash().to_string(),
        "replay_hash": host.replay_hash().to_string(),
    })
}

fn step<H: Host>(host: &mut H, args: &Args) -> Result<Value, String> {
    only(args, &["ticks", "inputs"])?;
    let ticks: u64 = typed(args, "ticks")?.unwrap_or(1);
    if !(1..=MAX_STEP_TICKS).contains(&ticks) {
        return Err(format!("ticks must be from 1 to {MAX_STEP_TICKS}"));
    }
    let inputs: Vec<Value> = typed(args, "inputs")?.unwrap_or_default();
    host.step(&inputs)?;
    for _ in 1..ticks {
        host.step(&[])?;
    }
    Ok(hashes(host))
}

fn list_components<H: Host>(host: &H) -> Value {
    let world = host.world();
    let components: Vec<Value> = world
        .component_names()
        .into_iter()
        .map(|name| json!({ "name": name, "count": world.component_count(name).unwrap_or(0) }))
        .collect();
    json!({ "tick": host.tick(), "entities": world.entity_count(), "components": components })
}

fn query<H: Host>(host: &H, args: &Args) -> Result<Value, String> {
    only(args, &["components", "limit"])?;
    let names: Vec<String> = required(args, "components")?;
    let limit: u64 = typed(args, "limit")?.unwrap_or(DEFAULT_QUERY_LIMIT);
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let matches = host
        .world()
        .query_names(&names)
        .map_err(|e| e.to_string())?;
    let entities: Vec<Value> = matches
        .iter()
        .take(usize::try_from(limit).unwrap_or(usize::MAX))
        .map(|&entity| entity_json(entity))
        .collect();
    Ok(json!({
        "tick": host.tick(),
        "total": matches.len(),
        "truncated": entities.len() < matches.len(),
        "entities": entities,
    }))
}

fn get_entity<H: Host>(host: &H, args: &Args) -> Result<Value, String> {
    only(args, &["index", "generation"])?;
    let entity = Entity::new(required(args, "index")?, required(args, "generation")?);
    let components = host.world().describe(entity).ok_or_else(|| {
        format!(
            "entity {}v{} is not alive",
            entity.index(),
            entity.generation()
        )
    })?;
    let components: Vec<Value> = components
        .into_iter()
        .map(|(name, fields)| {
            let fields: Map<String, Value> = fields
                .into_iter()
                .map(|(field, value)| (field.to_string(), field_json(value)))
                .collect();
            json!({ "name": name, "fields": fields })
        })
        .collect();
    Ok(json!({ "tick": host.tick(), "entity": entity_json(entity), "components": components }))
}

fn reset<H: Host>(host: &mut H, args: &Args) -> Result<Value, String> {
    only(args, &["seed"])?;
    let seed = match args.get("seed") {
        None => host.seed(),
        Some(Value::String(text)) => text
            .parse()
            .map_err(|_| format!("seed {text:?} is not an unsigned 64-bit decimal"))?,
        Some(_) => required(args, "seed")?,
    };
    host.reset(seed);
    Ok(hashes(host))
}

fn run_scenario<H: Host>(host: &H, args: &Args) -> Result<Value, String> {
    only(args, &["path", "scenario"])?;
    let parsed = match (args.get("path"), args.get("scenario")) {
        (Some(Value::String(path)), None) => scenario::load(Path::new(path)),
        (Some(_), None) => Err("argument \"path\" must be a string".to_string()),
        (None, Some(inline)) => scenario::from_json(inline),
        _ => Err("give exactly one of \"path\" or \"scenario\"".to_string()),
    }?;
    let mut fresh = host.fresh(parsed.seed);
    scenario::run(&mut fresh, &parsed).map(|report| report.to_json())
}

fn screenshot<H: Host>(host: &H, args: &Args) -> Result<(Value, Option<Value>), String> {
    let mut allowed = shot::SHOT_KEYS.to_vec();
    allowed.push("path");
    only(args, &allowed)?;
    let path: Option<String> = typed(args, "path")?;
    let shot = shot::from_args(args)?;
    let capture = shot::capture(host, &shot)?;
    let png = capture.image.to_png()?;
    if let Some(path) = &path {
        let path = Path::new(path);
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
        }
        std::fs::write(path, &png).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    }
    let mut structured = json!({
        "tick": host.tick(),
        "state_hash": host.state_hash().to_string(),
        "width": capture.image.width,
        "height": capture.image.height,
        "pixel_hash": capture.image.pixel_hash().to_string(),
        "camera": shot.camera.to_string(),
        "view_mode": shot.view_mode.name(),
        "show_grid": shot.show_grid,
        "off_screen": capture.off_screen,
        "adapter": capture.adapter,
        "bucket": capture.bucket,
    });
    if let Some(path) = path {
        structured["path"] = json!(path);
    }
    let image = json!({ "type": "image", "data": base64::encode(&png), "mimeType": "image/png" });
    Ok((structured, Some(image)))
}

fn entity_json(entity: Entity) -> Value {
    json!({ "index": entity.index(), "generation": entity.generation() })
}

fn field_json(value: FieldValue) -> Value {
    match value {
        FieldValue::Int(number) => json!(number),
        FieldValue::Bool(flag) => json!(flag),
        FieldValue::Fx(fixed) => json!(fixed.to_f64()),
        FieldValue::Text(text) => json!(text),
    }
}
