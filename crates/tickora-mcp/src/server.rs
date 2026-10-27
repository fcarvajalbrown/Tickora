use std::io::{self, BufRead, Write};

use serde_json::{Map, Value};

use crate::jsonrpc::{self, METHOD_NOT_FOUND, Message, RpcError};

#[derive(Default)]
pub struct Server {}

impl Server {
    pub fn new() -> Self {
        Self::default()
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

    fn request(&mut self, method: &str, _params: &Map<String, Value>) -> Result<Value, RpcError> {
        Err(RpcError::new(
            METHOD_NOT_FOUND,
            format!("unknown method {method}"),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(input: &str) -> Vec<Value> {
        let mut output = Vec::new();
        Server::new().serve(input.as_bytes(), &mut output).unwrap();
        String::from_utf8(output)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    #[test]
    fn loop_answers_requests_skips_notifications_and_survives_garbage() {
        let replies = run(concat!(
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"nope\"}\r\n",
            "\n",
            "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/whatever\"}\n",
            "garbage\n",
            "{\"jsonrpc\":\"2.0\",\"id\":\"b\",\"method\":\"nope\"}",
        ));
        assert_eq!(replies.len(), 3);
        assert_eq!(replies[0]["id"], 1);
        assert_eq!(replies[0]["error"]["code"], METHOD_NOT_FOUND);
        assert_eq!(replies[1]["error"]["code"], jsonrpc::PARSE_ERROR);
        assert_eq!(replies[2]["id"], "b");
    }

    #[test]
    fn invalid_utf8_is_a_parse_error_not_a_crash() {
        let mut output = Vec::new();
        Server::new()
            .serve(&b"\xff\xfe\n"[..], &mut output)
            .unwrap();
        let reply: Value = serde_json::from_slice(output.trim_ascii()).unwrap();
        assert_eq!(reply["error"]["code"], jsonrpc::PARSE_ERROR);
    }
}
