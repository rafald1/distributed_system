use std::collections::{HashMap, HashSet};
use std::io::{BufRead, Write};

use anyhow::Context;
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
struct Broadcast {
    msg_id: u64,
    message: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct BroadcastOk {
    msg_id: u64,
    in_reply_to: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct Read {
    msg_id: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct ReadOk {
    msg_id: u64,
    in_reply_to: u64,
    messages: HashSet<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Topology {
    msg_id: u64,
    topology: HashMap<String, Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TopologyOk {
    msg_id: u64,
    in_reply_to: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Body {
    Init(Init),
    InitOk(InitOk),
    Broadcast(Broadcast),
    BroadcastOk(BroadcastOk),
    Read(Read),
    ReadOk(ReadOk),
    Topology(Topology),
    TopologyOk(TopologyOk),
}

#[derive(Debug, Serialize, Deserialize)]
struct Message {
    src: String,
    dest: String,
    body: Body,
}

impl Message {
    fn process_received_message(&mut self, node: &mut Node) -> Vec<Self> {
        let mut responses: Vec<Message> = Vec::new();

        match &self.body {
            Body::Init(init_body) => {
                node.initialize(init_body.node_id.clone());

                responses.push(Self {
                    src: self.dest.clone(),
                    dest: self.src.clone(),
                    body: Body::InitOk(InitOk {
                        msg_id: node.incremented_msg_id(),
                        in_reply_to: init_body.msg_id,
                    }),
                });
            }

            Body::Broadcast(broadcast_body) => {
                (0..node.neighbours.len()).for_each(|i| {
                    responses.push(Self {
                        src: node.node_id.clone(),
                        dest: node.neighbours[i].clone(),
                        body: Body::Broadcast(Broadcast {
                            msg_id: node.incremented_msg_id(),
                            message: broadcast_body.message,
                        }),
                    })
                });
                
                node.messages.insert(broadcast_body.message);

                responses.push(Self {
                    src: self.dest.clone(),
                    dest: self.src.clone(),
                    body: Body::BroadcastOk(BroadcastOk {
                        msg_id: node.incremented_msg_id(),
                        in_reply_to: broadcast_body.msg_id,
                    }),
                });
            }

            Body::Read(read_body) => {
                responses.push(Self {
                    src: self.dest.clone(),
                    dest: self.src.clone(),
                    body: Body::ReadOk(ReadOk {
                        msg_id: node.incremented_msg_id(),
                        in_reply_to: read_body.msg_id,
                        messages: node.messages.clone(),
                    }),
                });
            }

            Body::Topology(topology_body) => {
                if let Some(neighbours) = topology_body.topology.get(&node.node_id) {
                    node.neighbours = neighbours.clone();
                }

                responses.push(Self {
                    src: self.dest.clone(),
                    dest: self.src.clone(),
                    body: Body::TopologyOk(TopologyOk {
                        msg_id: node.incremented_msg_id(),
                        in_reply_to: topology_body.msg_id,
                    }),
                });
            }

            Body::InitOk(_) | Body::BroadcastOk(_) | Body::ReadOk(_) | Body::TopologyOk(_) => {}
        }

        responses
    }
}

struct Node {
    node_id: String,
    msg_id: u64,
    messages: HashSet<u64>,
    neighbours: Vec<String>,
}

impl Node {
    fn new() -> Self {
        Self {
            node_id: String::new(),
            msg_id: 0,
            messages: HashSet::new(),
            neighbours: Vec::new(),
        }
    }

    fn initialize(&mut self, node_id: String) {
        self.node_id = node_id;
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

    let mut node = Node::new();

    while let Ok(line) = stdin
        .next()
        .context("Maelstrom should provide input to STDIN.")?
    {
        let mut msg: Message = serde_json::from_str(&line)
            .context("Failed to deserialize provided input to STDIN.")?;

        for message in msg.process_received_message(&mut node) {
            Node::send(&message, &mut stdout)?;
        }
    }

    Ok(())
}
