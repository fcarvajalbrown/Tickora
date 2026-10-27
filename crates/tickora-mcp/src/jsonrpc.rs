use serde_json::{Map, Value, json};

pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
pub const UNSUPPORTED_PROTOCOL_VERSION: i64 = -32022;

#[derive(Clone, Debug, PartialEq)]
pub enum Message {
    Request {
        id: Value,
        method: String,
        params: Map<String, Value>,
    },
    Notification {
        method: String,
    },
    Response,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

impl RpcError {
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    pub fn with_data(mut self, data: Value) -> Self {
        self.data = Some(data);
        self
    }

    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::new(INVALID_PARAMS, message)
    }
}

pub fn parse(line: &[u8]) -> Result<Message, Value> {
    let value: Value = serde_json::from_slice(line)
        .map_err(|e| failure(Value::Null, RpcError::new(PARSE_ERROR, e.to_string())))?;
    let Value::Object(mut object) = value else {
        return Err(failure(
            Value::Null,
            RpcError::new(INVALID_REQUEST, "message must be a JSON object"),
        ));
    };
    let id = object.remove("id");
    let reply_id = id.clone().filter(is_valid_id).unwrap_or(Value::Null);
    let invalid =
        |message: &str| failure(reply_id.clone(), RpcError::new(INVALID_REQUEST, message));
    if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Err(invalid("jsonrpc must be \"2.0\""));
    }
    let Some(method) = object.get("method") else {
        if object.contains_key("result") || object.contains_key("error") {
            return Ok(Message::Response);
        }
        return Err(invalid("missing method"));
    };
    let Some(method) = method.as_str().map(str::to_owned) else {
        return Err(invalid("method must be a string"));
    };
    let params = match object.remove("params") {
        None => Map::new(),
        Some(Value::Object(params)) => params,
        Some(_) => return Err(invalid("params must be an object")),
    };
    match id {
        None => Ok(Message::Notification { method }),
        Some(id) if is_valid_id(&id) => Ok(Message::Request { id, method, params }),
        Some(_) => Err(invalid("id must be a string or an integer")),
    }
}

pub fn success(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

pub fn failure(id: Value, error: RpcError) -> Value {
    let mut body = json!({ "code": error.code, "message": error.message });
    if let Some(data) = error.data {
        body["data"] = data;
    }
    json!({ "jsonrpc": "2.0", "id": id, "error": body })
}

fn is_valid_id(id: &Value) -> bool {
    id.is_string() || id.is_i64() || id.is_u64()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_request_with_params() {
        let message = parse(br#"{"jsonrpc":"2.0","id":7,"method":"ping","params":{"a":1}}"#);
        let Ok(Message::Request { id, method, params }) = message else {
            panic!("expected request, got {message:?}");
        };
        assert_eq!(id, json!(7));
        assert_eq!(method, "ping");
        assert_eq!(params["a"], json!(1));
    }

    #[test]
    fn a_message_without_id_is_a_notification() {
        let message = parse(br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
        assert_eq!(
            message,
            Ok(Message::Notification {
                method: "notifications/initialized".into()
            })
        );
    }

    #[test]
    fn client_responses_are_recognised() {
        assert_eq!(
            parse(br#"{"jsonrpc":"2.0","id":1,"result":{}}"#),
            Ok(Message::Response)
        );
    }

    #[test]
    fn malformed_json_is_a_parse_error_with_null_id() {
        let reply = parse(b"{not json").unwrap_err();
        assert_eq!(reply["id"], Value::Null);
        assert_eq!(reply["error"]["code"], json!(PARSE_ERROR));
    }

    #[test]
    fn null_or_fractional_ids_and_wrong_versions_are_invalid_requests() {
        for line in [
            &br#"{"jsonrpc":"2.0","id":null,"method":"ping"}"#[..],
            br#"{"jsonrpc":"2.0","id":1.5,"method":"ping"}"#,
            br#"{"jsonrpc":"1.0","id":1,"method":"ping"}"#,
            br#"{"jsonrpc":"2.0","id":1,"method":"ping","params":[1]}"#,
            br#"[{"jsonrpc":"2.0","id":1,"method":"ping"}]"#,
        ] {
            let reply = parse(line).unwrap_err();
            assert_eq!(reply["error"]["code"], json!(INVALID_REQUEST), "{reply}");
        }
    }

    #[test]
    fn error_data_is_only_present_when_set() {
        let plain = failure(json!(1), RpcError::new(METHOD_NOT_FOUND, "nope"));
        assert!(plain["error"].get("data").is_none());
        let rich = failure(
            json!(1),
            RpcError::invalid_params("bad").with_data(json!([1])),
        );
        assert_eq!(rich["error"]["data"], json!([1]));
    }
}
