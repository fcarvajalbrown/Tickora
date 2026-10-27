use serde::Deserialize;
use serde_json::{Map, Value, json};
use tickora_core::ecs::{Entity, FieldValue};

use crate::host::Host;
use crate::jsonrpc::RpcError;

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
    let outcome = match name {
        "tickora_step" => step(host, args),
        "tickora_state_hash" => only(args, &[]).map(|()| hashes(host)),
        "tickora_list_components" => only(args, &[]).map(|()| list_components(host)),
        "tickora_query" => query(host, args),
        "tickora_get_entity" => get_entity(host, args),
        "tickora_reset" => reset(host, args),
        _ => return Err(RpcError::invalid_params(format!("unknown tool {name}"))),
    };
    Ok(match outcome {
        Ok(structured) => json!({
            "content": [{ "type": "text", "text": structured.to_string() }],
            "structuredContent": structured,
            "isError": false,
        }),
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
