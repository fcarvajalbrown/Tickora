use std::io::{self, BufRead, Write};

use serde_json::{Map, Value, json};

use crate::host::Host;
use crate::jsonrpc::{self, METHOD_NOT_FOUND, Message, RpcError, UNSUPPORTED_PROTOCOL_VERSION};
use crate::tools;

pub const MODERN_VERSIONS: [&str; 1] = ["2026-07-28"];
pub const LEGACY_VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

pub const META_PROTOCOL_VERSION: &str = "io.modelcontextprotocol/protocolVersion";
pub const META_CLIENT_CAPABILITIES: &str = "io.modelcontextprotocol/clientCapabilities";
pub const META_SERVER_INFO: &str = "io.modelcontextprotocol/serverInfo";

const INSTRUCTIONS: &str = "Tickora deterministic simulation. Step it with tickora_step, read state with tickora_list_components, tickora_query and tickora_get_entity, and compare runs with tickora_state_hash. The same seed and the same call sequence always give the same hashes.";

pub struct Server<H> {
    host: H,
    legacy_version: Option<&'static str>,
}

impl<H: Host> Server<H> {
    pub fn new(host: H) -> Self {
        Self {
            host,
            legacy_version: None,
        }
    }

    pub fn host(&self) -> &H {
        &self.host
    }

    pub fn serve<R: BufRead, W: Write>(&mut self, mut reader: R, mut writer: W) -> io::Result<()> {
        let mut line = Vec::new();
        loop {
            line.clear();
            if reader.read_until(b'\n', &mut line)? == 0 {
                return Ok(());
            }
            let message = line.trim_ascii();
            if message.is_empty() {
                continue;
            }
            if let Some(reply) = self.handle_line(message) {
                serde_json::to_writer(&mut writer, &reply)?;
                writer.write_all(b"\n")?;
                writer.flush()?;
            }
        }
    }

    pub fn handle_line(&mut self, line: &[u8]) -> Option<Value> {
        match jsonrpc::parse(line) {
            Err(reply) => Some(reply),
            Ok(Message::Request { id, method, params }) => {
                Some(match self.request(&method, &params) {
                    Ok(result) => jsonrpc::success(id, result),
                    Err(error) => jsonrpc::failure(id, error),
                })
            }
            Ok(Message::Notification { .. } | Message::Response) => None,
        }
    }

    fn request(&mut self, method: &str, params: &Map<String, Value>) -> Result<Value, RpcError> {
        let meta = params.get("_meta").and_then(Value::as_object);
        match meta.and_then(|meta| meta.get(META_PROTOCOL_VERSION)) {
            Some(version) => self.modern(version, meta.unwrap_or(&Map::new()), method, params),
            None => self.legacy(method, params),
        }
    }

    fn modern(
        &mut self,
        version: &Value,
        meta: &Map<String, Value>,
        method: &str,
        params: &Map<String, Value>,
    ) -> Result<Value, RpcError> {
        let version = version.as_str().ok_or_else(|| {
            RpcError::invalid_params(format!("{META_PROTOCOL_VERSION} must be a string"))
        })?;
        if !MODERN_VERSIONS.contains(&version) {
            return Err(unsupported_version(version));
        }
        if !meta
            .get(META_CLIENT_CAPABILITIES)
            .is_some_and(Value::is_object)
        {
            return Err(RpcError::invalid_params(format!(
                "missing {META_CLIENT_CAPABILITIES} in _meta"
            )));
        }
        let mut result = match method {
            "server/discover" => json!({
                "supportedVersions": MODERN_VERSIONS,
                "capabilities": capabilities(),
                "instructions": INSTRUCTIONS,
            }),
            "ping" => json!({}),
            "tools/list" => json!({ "tools": tools::definitions(&self.host) }),
            "tools/call" => tools::call(&mut self.host, params)?,
            _ => return Err(method_not_found(method)),
        };
        result["resultType"] = json!("complete");
        result["_meta"] = json!({ META_SERVER_INFO: server_info() });
        Ok(result)
    }

    fn legacy(&mut self, method: &str, params: &Map<String, Value>) -> Result<Value, RpcError> {
        match method {
            "initialize" => {
                let requested = params
                    .get("protocolVersion")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RpcError::invalid_params("missing protocolVersion"))?;
                let version = LEGACY_VERSIONS
                    .into_iter()
                    .find(|&known| known == requested)
                    .unwrap_or(LEGACY_VERSIONS[0]);
                self.legacy_version = Some(version);
                Ok(json!({
                    "protocolVersion": version,
                    "capabilities": capabilities(),
                    "serverInfo": server_info(),
                    "instructions": INSTRUCTIONS,
                }))
            }
            "ping" => Ok(json!({})),
            _ if self.legacy_version.is_none() => Err(RpcError::invalid_params(format!(
                "missing {META_PROTOCOL_VERSION} in _meta; send it on every request or call initialize first"
            ))),
            "tools/list" => Ok(json!({ "tools": tools::definitions(&self.host) })),
            "tools/call" => tools::call(&mut self.host, params),
            _ => Err(method_not_found(method)),
        }
    }
}

fn capabilities() -> Value {
    json!({ "tools": {} })
}

fn server_info() -> Value {
    json!({ "name": env!("CARGO_PKG_NAME"), "version": env!("CARGO_PKG_VERSION") })
}

fn method_not_found(method: &str) -> RpcError {
    RpcError::new(METHOD_NOT_FOUND, format!("unknown method {method}"))
}

fn unsupported_version(requested: &str) -> RpcError {
    let supported: Vec<&str> = MODERN_VERSIONS.into_iter().chain(LEGACY_VERSIONS).collect();
    RpcError::new(UNSUPPORTED_PROTOCOL_VERSION, "Unsupported protocol version")
        .with_data(json!({ "supported": supported, "requested": requested }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::ArenaHost;

    fn run(input: &str) -> Vec<Value> {
        let mut output = Vec::new();
        Server::new(ArenaHost::new(0))
            .serve(input.as_bytes(), &mut output)
            .unwrap();
        String::from_utf8(output)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    fn modern_meta(version: &str) -> Value {
        json!({ META_PROTOCOL_VERSION: version, META_CLIENT_CAPABILITIES: {} })
    }

    fn request(id: u64, method: &str, params: Value) -> String {
        json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string() + "\n"
    }

    #[test]
    fn loop_answers_requests_skips_notifications_and_survives_garbage() {
        let replies = run(concat!(
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\r\n",
            "\n",
            "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/whatever\"}\n",
            "garbage\n",
            "{\"jsonrpc\":\"2.0\",\"id\":\"b\",\"method\":\"ping\"}",
        ));
        assert_eq!(replies.len(), 3);
        assert_eq!(replies[0]["id"], 1);
        assert_eq!(replies[0]["result"], json!({}));
        assert_eq!(replies[1]["error"]["code"], jsonrpc::PARSE_ERROR);
        assert_eq!(replies[2]["id"], "b");
    }

    #[test]
    fn invalid_utf8_is_a_parse_error_not_a_crash() {
        let mut output = Vec::new();
        Server::new(ArenaHost::new(0))
            .serve(&b"\xff\xfe\n"[..], &mut output)
            .unwrap();
        let reply: Value = serde_json::from_slice(output.trim_ascii()).unwrap();
        assert_eq!(reply["error"]["code"], jsonrpc::PARSE_ERROR);
    }

    #[test]
    fn discover_lists_modern_versions_and_identifies_the_server() {
        let replies = run(&request(
            1,
            "server/discover",
            json!({ "_meta": modern_meta("2026-07-28") }),
        ));
        let result = &replies[0]["result"];
        assert_eq!(result["resultType"], "complete");
        assert_eq!(result["supportedVersions"], json!(["2026-07-28"]));
        assert_eq!(result["capabilities"], json!({ "tools": {} }));
        assert_eq!(result["_meta"][META_SERVER_INFO]["name"], "tickora-mcp");
    }

    #[test]
    fn an_unknown_modern_version_gets_the_supported_list() {
        let replies = run(&request(
            1,
            "server/discover",
            json!({ "_meta": modern_meta("2099-01-01") }),
        ));
        let error = &replies[0]["error"];
        assert_eq!(error["code"], UNSUPPORTED_PROTOCOL_VERSION);
        assert_eq!(error["data"]["requested"], "2099-01-01");
        assert_eq!(error["data"]["supported"][0], "2026-07-28");
        assert!(
            error["data"]["supported"]
                .as_array()
                .unwrap()
                .contains(&json!("2025-11-25"))
        );
    }

    #[test]
    fn a_modern_request_without_client_capabilities_is_invalid_params() {
        let replies = run(&request(
            1,
            "server/discover",
            json!({ "_meta": { META_PROTOCOL_VERSION: "2026-07-28" } }),
        ));
        assert_eq!(replies[0]["error"]["code"], jsonrpc::INVALID_PARAMS);
    }

    #[test]
    fn a_request_with_neither_meta_nor_initialize_is_rejected() {
        let replies = run(&request(1, "server/discover", json!({})));
        assert_eq!(replies[0]["error"]["code"], jsonrpc::INVALID_PARAMS);
    }

    #[test]
    fn initialize_echoes_a_known_legacy_version_and_falls_back_to_the_newest() {
        let replies = run(&(request(
            1,
            "initialize",
            json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "t", "version": "0" } }),
        ) + &request(
            2,
            "initialize",
            json!({ "protocolVersion": "1999-01-01", "capabilities": {} }),
        )));
        assert_eq!(replies[0]["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(replies[0]["result"]["serverInfo"]["name"], "tickora-mcp");
        assert!(replies[0]["result"].get("resultType").is_none());
        assert_eq!(replies[1]["result"]["protocolVersion"], "2025-11-25");
    }

    #[test]
    fn after_initialize_unknown_methods_are_method_not_found() {
        let replies = run(
            &(request(1, "initialize", json!({ "protocolVersion": "2025-11-25" }))
                + "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n"
                + &request(2, "nope", json!({}))),
        );
        assert_eq!(replies.len(), 2);
        assert_eq!(replies[1]["error"]["code"], METHOD_NOT_FOUND);
    }
}
