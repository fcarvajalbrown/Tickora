use serde_json::{Map, Value};
use tickora_core::ecs::Entity;

pub const MAX_RUN_TICKS: u64 = 1_000_000;

const CHECK_KEYS: [&str; 6] = [
    "state_hash",
    "replay_hash",
    "entities",
    "component",
    "query",
    "entity",
];

#[derive(Clone, Debug, PartialEq)]
pub struct Scenario {
    pub name: Option<String>,
    pub seed: u64,
    pub inputs: Vec<Batch>,
    pub run_to: u64,
    pub expect: Vec<Assertion>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Batch {
    pub at: u64,
    pub inputs: Vec<Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Assertion {
    pub at: u64,
    pub check: Check,
    pub source: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Check {
    StateHash(u64),
    ReplayHash(u64),
    Entities(Range),
    Component {
        name: String,
        count: Range,
    },
    Query {
        names: Vec<String>,
        count: Range,
    },
    Field {
        entity: Entity,
        component: String,
        field: String,
        value: Expected,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Range {
    pub min: Option<i64>,
    pub max: Option<i64>,
}

impl Range {
    pub fn contains(self, value: i64) -> bool {
        self.min.is_none_or(|min| value >= min) && self.max.is_none_or(|max| value <= max)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expected {
    Int(Range),
    Literal(Value),
    Raw(Range),
    Approx { target: f64, tol: f64 },
}

pub fn parse(text: &str) -> Result<Scenario, String> {
    let value: Value = serde_json::from_str(text).map_err(|e| format!("invalid JSON: {e}"))?;
    from_json(&value)
}

pub fn from_json(value: &Value) -> Result<Scenario, String> {
    let root = object(value, "scenario")?;
    only(
        root,
        "scenario",
        &["name", "seed", "inputs", "run_to", "expect"],
    )?;
    let name = match root.get("name") {
        None => None,
        Some(Value::String(name)) => Some(name.clone()),
        Some(_) => return Err("name must be a string".to_string()),
    };
    let seed = seed(need(root, "scenario", "seed")?)?;
    let run_to = unsigned(need(root, "scenario", "run_to")?, "run_to")?;
    if run_to > MAX_RUN_TICKS {
        return Err(format!("run_to must be at most {MAX_RUN_TICKS}"));
    }
    let inputs = match root.get("inputs") {
        None => Vec::new(),
        Some(value) => array(value, "inputs")?
            .iter()
            .enumerate()
            .map(|(i, batch)| parse_batch(batch, &format!("inputs[{i}]")))
            .collect::<Result<Vec<_>, _>>()?,
    };
    for pair in inputs.windows(2) {
        if pair[1].at <= pair[0].at {
            return Err(format!(
                "inputs ticks must be strictly increasing: {} follows {}",
                pair[1].at, pair[0].at
            ));
        }
    }
    if let Some(last) = inputs.last().filter(|batch| batch.at >= run_to) {
        return Err(format!(
            "inputs at {} never run: run_to is {run_to}",
            last.at
        ));
    }
    let expect = array(need(root, "scenario", "expect")?, "expect")?
        .iter()
        .enumerate()
        .map(|(i, entry)| parse_assertion(entry, &format!("expect[{i}]")))
        .collect::<Result<Vec<_>, _>>()?;
    if expect.is_empty() {
        return Err("expect needs at least one assertion".to_string());
    }
    if let Some(late) = expect.iter().find(|assertion| assertion.at > run_to) {
        return Err(format!("assertion at {} is past run_to {run_to}", late.at));
    }
    Ok(Scenario {
        name,
        seed,
        inputs,
        run_to,
        expect,
    })
}

fn parse_batch(value: &Value, path: &str) -> Result<Batch, String> {
    let batch = object(value, path)?;
    only(batch, path, &["at", "inputs"])?;
    Ok(Batch {
        at: unsigned(need(batch, path, "at")?, &join(path, "at"))?,
        inputs: array(need(batch, path, "inputs")?, &join(path, "inputs"))?.clone(),
    })
}

fn parse_assertion(value: &Value, path: &str) -> Result<Assertion, String> {
    let entry = object(value, path)?;
    let at = unsigned(need(entry, path, "at")?, &join(path, "at"))?;
    let present: Vec<&str> = CHECK_KEYS
        .into_iter()
        .filter(|key| entry.contains_key(*key))
        .collect();
    let [kind] = present[..] else {
        return Err(format!(
            "{path}: needs exactly one of {}",
            CHECK_KEYS.join(", ")
        ));
    };
    let get = |key: &str| need(entry, path, key);
    let at_path = |key: &str| join(path, key);
    let check = match kind {
        "state_hash" | "replay_hash" => {
            only(entry, path, &["at", kind])?;
            let hash = decimal(get(kind)?, &at_path(kind))?;
            if kind == "state_hash" {
                Check::StateHash(hash)
            } else {
                Check::ReplayHash(hash)
            }
        }
        "entities" => {
            only(entry, path, &["at", "entities"])?;
            Check::Entities(range(get("entities")?, &at_path("entities"))?)
        }
        "component" => {
            only(entry, path, &["at", "component", "count"])?;
            Check::Component {
                name: string(get("component")?, &at_path("component"))?,
                count: range(get("count")?, &at_path("count"))?,
            }
        }
        "query" => {
            only(entry, path, &["at", "query", "count"])?;
            let names = array(get("query")?, &at_path("query"))?
                .iter()
                .enumerate()
                .map(|(i, name)| string(name, &format!("{path}.query[{i}]")))
                .collect::<Result<Vec<_>, _>>()?;
            Check::Query {
                names,
                count: range(get("count")?, &at_path("count"))?,
            }
        }
        _ => {
            only(entry, path, &["at", "entity", "field", "value"])?;
            let field_path = at_path("field");
            let target = string(get("field")?, &field_path)?;
            let (component, field) = target
                .split_once('.')
                .filter(|(component, field)| !component.is_empty() && !field.is_empty())
                .ok_or_else(|| format!("{field_path}: must be \"component.field\""))?;
            Check::Field {
                entity: entity(get("entity")?, &at_path("entity"))?,
                component: component.to_string(),
                field: field.to_string(),
                value: expected(get("value")?, &at_path("value"))?,
            }
        }
    };
    Ok(Assertion {
        at,
        check,
        source: value.clone(),
    })
}

fn entity(value: &Value, path: &str) -> Result<Entity, String> {
    let map = object(value, path)?;
    only(map, path, &["index", "generation"])?;
    let part = |key: &str| {
        let key_path = join(path, key);
        u32::try_from(unsigned(need(map, path, key)?, &key_path)?)
            .map_err(|_| format!("{key_path}: must be at most {}", u32::MAX))
    };
    Ok(Entity::new(part("index")?, part("generation")?))
}

fn range(value: &Value, path: &str) -> Result<Range, String> {
    let map = object(value, path)?;
    only(map, path, &["eq", "min", "max"])?;
    bounds(map, path, ["eq", "min", "max"], |value, path| {
        value
            .as_i64()
            .ok_or_else(|| format!("{path}: must be a signed 64-bit integer"))
    })
}

fn expected(value: &Value, path: &str) -> Result<Expected, String> {
    let map = object(value, path)?;
    if ["raw", "raw_min", "raw_max"]
        .iter()
        .any(|key| map.contains_key(*key))
    {
        only(map, path, &["raw", "raw_min", "raw_max"])?;
        return bounds(map, path, ["raw", "raw_min", "raw_max"], |value, path| {
            value
                .as_str()
                .and_then(|text| text.parse().ok())
                .ok_or_else(|| format!("{path}: must be a signed 64-bit decimal string"))
        })
        .map(Expected::Raw);
    }
    if map.contains_key("approx") {
        only(map, path, &["approx", "tol"])?;
        let number = |key: &str| {
            need(map, path, key)?
                .as_f64()
                .filter(|number| number.is_finite())
                .ok_or_else(|| format!("{}: must be a finite number", join(path, key)))
        };
        let tol = number("tol")?;
        if tol < 0.0 {
            return Err(format!("{path}.tol: must not be negative"));
        }
        return Ok(Expected::Approx {
            target: number("approx")?,
            tol,
        });
    }
    match map.get("eq") {
        Some(literal @ (Value::Bool(_) | Value::String(_))) => {
            only(map, path, &["eq"])?;
            Ok(Expected::Literal(literal.clone()))
        }
        _ => range(value, path).map(Expected::Int),
    }
}

fn bounds(
    map: &Map<String, Value>,
    path: &str,
    [eq, min, max]: [&str; 3],
    read: impl Fn(&Value, &str) -> Result<i64, String>,
) -> Result<Range, String> {
    let get = |key: &str| {
        map.get(key)
            .map(|value| read(value, &join(path, key)))
            .transpose()
    };
    let range = match (get(eq)?, get(min)?, get(max)?) {
        (Some(_), Some(_), _) | (Some(_), _, Some(_)) => {
            return Err(format!(
                "{path}: {eq} cannot be combined with {min} or {max}"
            ));
        }
        (Some(value), None, None) => Range {
            min: Some(value),
            max: Some(value),
        },
        (None, None, None) => return Err(format!("{path}: needs {eq}, {min} or {max}")),
        (None, min, max) => Range { min, max },
    };
    match range {
        Range {
            min: Some(low),
            max: Some(high),
        } if low > high => Err(format!("{path}: {min} {low} is above {max} {high}")),
        _ => Ok(range),
    }
}

fn seed(value: &Value) -> Result<u64, String> {
    match value {
        Value::String(_) => decimal(value, "seed"),
        _ => unsigned(value, "seed"),
    }
}

fn decimal(value: &Value, path: &str) -> Result<u64, String> {
    value
        .as_str()
        .and_then(|text| text.parse().ok())
        .ok_or_else(|| format!("{path}: must be an unsigned 64-bit decimal string"))
}

fn unsigned(value: &Value, path: &str) -> Result<u64, String> {
    value
        .as_u64()
        .ok_or_else(|| format!("{path}: must be an unsigned integer"))
}

fn string(value: &Value, path: &str) -> Result<String, String> {
    value
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| format!("{path}: must be a string"))
}

fn array<'a>(value: &'a Value, path: &str) -> Result<&'a Vec<Value>, String> {
    value
        .as_array()
        .ok_or_else(|| format!("{path}: must be an array"))
}

fn object<'a>(value: &'a Value, path: &str) -> Result<&'a Map<String, Value>, String> {
    value
        .as_object()
        .ok_or_else(|| format!("{path}: must be an object"))
}

fn need<'a>(map: &'a Map<String, Value>, path: &str, key: &str) -> Result<&'a Value, String> {
    map.get(key)
        .ok_or_else(|| format!("{path}: missing key {key:?}"))
}

fn only(map: &Map<String, Value>, path: &str, allowed: &[&str]) -> Result<(), String> {
    match map.keys().find(|key| !allowed.contains(&key.as_str())) {
        Some(key) => Err(format!("{path}: unexpected key {key:?}")),
        None => Ok(()),
    }
}

fn join(path: &str, key: &str) -> String {
    format!("{path}.{key}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn minimal(expect: Value) -> Value {
        json!({ "seed": 7, "run_to": 10, "expect": [expect] })
    }

    fn error(value: Value) -> String {
        from_json(&value).unwrap_err()
    }

    #[test]
    fn a_full_scenario_parses_every_check_kind() {
        let scenario = from_json(&json!({
            "name": "all kinds",
            "seed": "18446744073709551615",
            "inputs": [
                { "at": 0, "inputs": [{ "type": "spawn" }] },
                { "at": 4, "inputs": [] },
            ],
            "run_to": 5,
            "expect": [
                { "at": 5, "state_hash": "1" },
                { "at": 5, "replay_hash": "2" },
                { "at": 0, "entities": { "eq": 0 } },
                { "at": 1, "component": "health", "count": { "min": 1 } },
                { "at": 1, "query": [], "count": { "min": 0, "max": 3 } },
                { "at": 1, "entity": { "index": 0, "generation": 0 }, "field": "health.hp", "value": { "max": 100 } },
                { "at": 1, "entity": { "index": 0, "generation": 0 }, "field": "position.x", "value": { "raw": "-5" } },
                { "at": 1, "entity": { "index": 0, "generation": 0 }, "field": "position.y", "value": { "approx": 1.5, "tol": 0.01 } },
                { "at": 1, "entity": { "index": 0, "generation": 0 }, "field": "tag.name", "value": { "eq": "hero" } },
            ],
        }))
        .unwrap();
        assert_eq!(scenario.name.as_deref(), Some("all kinds"));
        assert_eq!(scenario.seed, u64::MAX);
        assert_eq!(scenario.inputs.len(), 2);
        let checks: Vec<&Check> = scenario.expect.iter().map(|a| &a.check).collect();
        assert_eq!(checks[0], &Check::StateHash(1));
        assert_eq!(checks[1], &Check::ReplayHash(2));
        let exactly_zero = Range {
            min: Some(0),
            max: Some(0),
        };
        assert_eq!(checks[2], &Check::Entities(exactly_zero));
        let Check::Field { value, .. } = checks[6] else {
            panic!("{:?}", checks[6]);
        };
        assert_eq!(
            value,
            &Expected::Raw(Range {
                min: Some(-5),
                max: Some(-5)
            })
        );
        let Check::Field { value, .. } = checks[8] else {
            panic!("{:?}", checks[8]);
        };
        assert_eq!(value, &Expected::Literal(json!("hero")));
    }

    #[test]
    fn invalid_json_and_wrong_shapes_are_rejected() {
        assert!(parse("{").unwrap_err().starts_with("invalid JSON"));
        assert!(error(json!([])).contains("must be an object"));
        assert!(error(json!({ "seed": 1, "run_to": 1 })).contains("missing key \"expect\""));
        assert!(
            error(json!({ "seed": 1, "run_to": 1, "expect": [], "extra": 0 }))
                .contains("unexpected key \"extra\"")
        );
    }

    #[test]
    fn seeds_and_hashes_need_unsigned_64_bit_values() {
        assert!(error(json!({ "seed": -1, "run_to": 1, "expect": [] })).contains("seed"));
        assert!(error(json!({ "seed": "1x", "run_to": 1, "expect": [] })).contains("seed"));
        assert!(error(minimal(json!({ "at": 1, "state_hash": 5 }))).contains("decimal string"));
    }

    #[test]
    fn tick_limits_are_enforced() {
        assert!(
            error(json!({ "seed": 0, "run_to": MAX_RUN_TICKS + 1, "expect": [] }))
                .contains("at most")
        );
        assert!(
            error(minimal(json!({ "at": 11, "entities": { "eq": 0 } }))).contains("past run_to")
        );
        let late_input = json!({
            "seed": 0, "run_to": 3,
            "inputs": [{ "at": 3, "inputs": [] }],
            "expect": [{ "at": 0, "entities": { "eq": 0 } }],
        });
        assert!(error(late_input).contains("never run"));
        let unordered = json!({
            "seed": 0, "run_to": 9,
            "inputs": [{ "at": 2, "inputs": [] }, { "at": 2, "inputs": [] }],
            "expect": [{ "at": 0, "entities": { "eq": 0 } }],
        });
        assert!(error(unordered).contains("strictly increasing"));
        assert!(error(json!({ "seed": 0, "run_to": 1, "expect": [] })).contains("at least one"));
    }

    #[test]
    fn an_assertion_needs_exactly_one_check_and_only_its_keys() {
        assert!(error(minimal(json!({ "at": 1 }))).contains("exactly one"));
        let two = json!({ "at": 1, "state_hash": "1", "replay_hash": "1" });
        assert!(error(minimal(two)).contains("exactly one"));
        let stray = json!({ "at": 1, "state_hash": "1", "count": { "eq": 1 } });
        assert!(error(minimal(stray)).contains("unexpected key \"count\""));
    }

    #[test]
    fn comparisons_reject_contradictions() {
        let both = json!({ "at": 1, "entities": { "eq": 1, "min": 0 } });
        assert!(error(minimal(both)).contains("cannot be combined"));
        let empty = json!({ "at": 1, "entities": {} });
        assert!(error(minimal(empty)).contains("needs eq, min or max"));
        let inverted = json!({ "at": 1, "entities": { "min": 3, "max": 2 } });
        assert!(error(minimal(inverted)).contains("above"));
        let field = |value: Value| {
            minimal(
                json!({ "at": 1, "entity": { "index": 0, "generation": 0 }, "field": "a.b", "value": value }),
            )
        };
        assert!(error(field(json!({ "raw": 5 }))).contains("decimal string"));
        assert!(error(field(json!({ "approx": 1.0 }))).contains("missing key \"tol\""));
        assert!(error(field(json!({ "approx": 1.0, "tol": -1.0 }))).contains("negative"));
        assert!(error(field(json!({ "eq": true, "min": 0 }))).contains("unexpected key"));
    }

    #[test]
    fn field_targets_need_component_dot_field_and_a_valid_entity() {
        let target = |field: &str, entity: Value| {
            minimal(json!({ "at": 1, "entity": entity, "field": field, "value": { "eq": 1 } }))
        };
        let zero = json!({ "index": 0, "generation": 0 });
        assert!(error(target("hp", zero.clone())).contains("component.field"));
        assert!(error(target(".hp", zero)).contains("component.field"));
        let huge = json!({ "index": u64::from(u32::MAX) + 1, "generation": 0 });
        assert!(error(target("health.hp", huge)).contains("at most"));
    }

    #[test]
    fn ranges_include_their_bounds() {
        let range = Range {
            min: Some(2),
            max: Some(4),
        };
        assert!(!range.contains(1));
        assert!(range.contains(2));
        assert!(range.contains(4));
        assert!(!range.contains(5));
        let open = Range {
            min: None,
            max: None,
        };
        assert!(open.contains(i64::MIN));
    }
}
