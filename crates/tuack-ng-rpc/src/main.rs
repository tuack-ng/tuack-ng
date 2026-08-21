mod adapter;
mod jsonrpc;
mod output;
mod protocol;
mod server;
mod session;

use std::sync::Arc;
use std::sync::atomic::Ordering;

use tokio::io::{AsyncBufReadExt, BufReader};

use crate::jsonrpc::{Id, Incoming, PARSE_ERROR, Response, RpcError};
use crate::output::write_line;
use crate::server::Server;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let server = Arc::new(Server::new()?);
    let exit_flag = server.exit_flag();

    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<Incoming>(trimmed) {
            Ok(msg) => {
                if let Some(resp) = server.handle(msg).await {
                    let s = serde_json::to_string(&resp).unwrap_or_default();
                    write_line(&s);
                }
            }
            Err(_) => {
                let resp = Response::err(Id::Null, RpcError::new(PARSE_ERROR, "解析错误"));
                let s = serde_json::to_string(&resp).unwrap_or_default();
                write_line(&s);
            }
        }
        if exit_flag.load(Ordering::SeqCst) {
            break;
        }
    }
    Ok(())
}
