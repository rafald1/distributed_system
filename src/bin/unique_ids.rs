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
struct Generate {
    msg_id: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct GenerateOk {
    msg_id: u64,
    in_reply_to: u64,
    r#id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
enum Body {
    Init(Init),
    InitOk(InitOk),
    Generate(Generate),
    GenerateOk(GenerateOk),
}

#[derive(Debug, Serialize, Deserialize)]
struct Message {
    src: String,
    dest: String,
    body: Body,
}

impl Message {
    fn prepare_reply(&self, node: &mut Node) -> Option<Self> {
        let body: Option<Body> = match &self.body {
            Body::Init(init_body) => Some(Body::InitOk(InitOk {
                msg_id: node.incremented_msg_id(),
                in_reply_to: init_body.msg_id,
            })),

            Body::Generate(generate_body) => Some(Body::GenerateOk(GenerateOk {
                msg_id: node.incremented_msg_id(),
                in_reply_to: generate_body.msg_id,
                r#id: format!("{}_{}", node.node_id, node.msg_id),
            })),

            Body::InitOk(_) | Body::GenerateOk(_) => None,
        };

        let body = body?;

        Some(Self {
            src: self.dest.clone(),
            dest: self.src.clone(),
            body,
        })
    }
}

struct Node {
    #[allow(dead_code)]
    node_id: String,
    msg_id: u64,
}

impl Node {
    fn initialize(node_id: String) -> Self {
        Self { node_id, msg_id: 0 }
    }

    fn incremented_msg_id(&mut self) -> u64 {
        self.msg_id += 1;
        self.msg_id
    }

    fn send<W: Write>(msg: &Message, writer: &mut W) -> Result<(), anyhow::Error> {
        serde_json::to_writer(&mut *writer, &msg).context("Failed to serialize reply message")?;
        writer.write_all(b"\n").context("Failed to write newline")?;
        Ok(())
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
    let mut node = Node::initialize(init_body.node_id.clone());

    let init_reply = init_msg
        .prepare_reply(&mut node)
        .context("Failed to prepare InitOk message")?;

    Node::send(&init_reply, &mut stdout)?;

    while let Ok(line) = stdin
        .next()
        .context("Maelstrom should provide input to STDIN.")?
    {
        let msg: Message = serde_json::from_str(&line)
            .context("Failed to deserialize provided input to STDIN.")?;

        if let Some(reply) = msg.prepare_reply(&mut node) {
            Node::send(&reply, &mut stdout)?;
        } else {
            eprintln!("No reply was prepared for message: {:?}", msg);
        };
    }

    Ok(())
}
