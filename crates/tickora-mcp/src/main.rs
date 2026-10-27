use std::io;

use tickora_mcp::Server;
use tickora_mcp::demo::ArenaHost;

fn main() -> io::Result<()> {
    Server::new(ArenaHost::new(0)).serve(io::stdin().lock(), io::stdout().lock())
}
