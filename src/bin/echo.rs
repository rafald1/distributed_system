use std::io::{BufRead, Write};

use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct Init {
    msg_id: u64,
    node_id: String,
    node_ids: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct InitOk {
    msg_id: u64,
    in_reply_to: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct Echo {
    msg_id: u64,
    echo: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct EchoOk {
    msg_id: u64,
    in_reply_to: u64,
    echo: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")] // This adds a `type` field in the serialized JSON based on the variant name
#[serde(rename_all = "snake_case")]
enum Body {
    Init(Init),
    InitOk(InitOk),
    Echo(Echo),
    EchoOk(EchoOk),
}

#[derive(Debug, Serialize, Deserialize)]
struct Message {
    src: String,
    dest: String,
    body: Body,
}

impl Message {
    fn prepare_reply(&self, msg_id: u64) -> Option<Self> {
        let body: Option<Body> = match &self.body {
            Body::Init(init_body) => Some(Body::InitOk(InitOk {
                msg_id,
                in_reply_to: init_body.msg_id,
            })),

            Body::Echo(echo_body) => Some(Body::EchoOk(EchoOk {
                msg_id,
                in_reply_to: echo_body.msg_id,
                echo: echo_body.echo.clone(),
            })),

            Body::InitOk(_) | Body::EchoOk(_) => None,
        };

        let body = body?;

        Some(Self {
            src: self.dest.clone(),
            dest: self.src.clone(),
            body,
        })
    }
}

struct EchoServer {
    #[allow(dead_code)]
    node_id: String,
    msg_id: u64,
}

impl EchoServer {
    fn initialize(node_id: String) -> Self {
        Self { node_id, msg_id: 0 }
    }

    fn incremented_msg_id(&mut self) -> u64 {
        self.msg_id += 1;
        self.msg_id
    }
}

fn main() -> Result<(), anyhow::Error> {
    let stdin = std::io::stdin().lock();
    let mut stdin = stdin.lines();
    let mut stdout = std::io::stdout().lock();

    let init_msg: Message = serde_json::from_str(
        &stdin
            .next()
            .context("Maelstrom should provide input to STDIN.")?
            .context("Failed to read message from stdin.")?,
    )
    .context("Failed to deserialize provided input to STDIN.")?;

    let Body::Init(ref init_body) = init_msg.body else {
        bail!("Expected Init message as the first received message.");
    };

    let mut node = EchoServer::initialize(init_body.node_id.clone());

    let init_reply = init_msg
        .prepare_reply(node.incremented_msg_id())
        .context("Failed to prepare InitOk message")?;

    serde_json::to_writer(&mut stdout, &init_reply)
        .context("Failed to serialize InitOk message")?;
    stdout.write_all(b"\n").context("Failed to write newline")?;

    while let Ok(line) = stdin
        .next()
        .context("Maelstrom should provide input to STDIN.")?
    {
        let msg: Message = serde_json::from_str(&line)
            .context("Failed to deserialize provided input to STDIN.")?;

        if let Some(reply) = msg.prepare_reply(node.incremented_msg_id()) {
            serde_json::to_writer(&mut stdout, &reply)
                .context("Failed to serialize reply message")?;
            stdout.write_all(b"\n").context("Failed to write newline")?;
        } else {
            eprintln!("No reply was prepared for message: {:?}", msg);
        };
    }

    Ok(())
}
