use std::collections::HashMap;
use std::io::{BufRead, Write};

use anyhow::Context;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Body {
    Init {
        msg_id: u64,
        node_id: String,
        node_ids: Vec<String>,
    },

    InitOk {
        msg_id: u64,
        in_reply_to: u64,
    },

    Add {
        msg_id: u64,
        delta: u64,
    },

    AddOk {
        msg_id: u64,
        in_reply_to: u64,
    },

    Read {
        msg_id: u64,
    },

    ReadOk {
        msg_id: u64,
        in_reply_to: u64,
        value: u64,
    },

    Sync {
        msg_id: u64,
        counters: HashMap<String, u64>,
    },
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

        let build_message_from = |body: Body| -> Self {
            Self {
                src: self.dest.clone(),
                dest: self.src.clone(),
                body,
            }
        };

        match &mut self.body {
            Body::Init {
                msg_id,
                node_id,
                node_ids,
            } => {
                node.initialize(node_id.clone(), node_ids);

                responses.push(build_message_from(Body::InitOk {
                    msg_id: node.incremented_msg_id(),
                    in_reply_to: *msg_id,
                }));
            }

            Body::Add { msg_id, delta } => {
                node.counters
                    .entry(node.node_id.clone())
                    .and_modify(|value| *value += *delta);

                let incremented_msg_id = node.incremented_msg_id();

                for destination_node in node.cluster.iter().filter(|&id| *id != node.node_id) {
                    responses.push(Message {
                        src: node.node_id.clone(),
                        dest: destination_node.clone(),
                        body: Body::Sync {
                            msg_id: incremented_msg_id,
                            counters: node.counters.clone(),
                        },
                    });
                }

                responses.push(build_message_from(Body::AddOk {
                    msg_id: node.incremented_msg_id(),
                    in_reply_to: *msg_id,
                }));
            }

            Body::Read { msg_id } => {
                responses.push(build_message_from(Body::ReadOk {
                    msg_id: node.incremented_msg_id(),
                    in_reply_to: *msg_id,
                    value: node.counters.values().sum::<u64>(),
                }));
            }

            Body::Sync { counters, .. } => {
                for (key, &remote_value) in counters.iter() {
                    node.counters.entry(key.clone()).and_modify(|local_value| {
                        if remote_value > *local_value {
                            *local_value = remote_value;
                        }
                    });
                }
            }

            Body::InitOk { .. } | Body::AddOk { .. } | Body::ReadOk { .. } => {}
        }

        responses
    }
}

struct Node {
    node_id: String,
    cluster: Vec<String>,
    msg_id: u64,
    counters: HashMap<String, u64>,
}

impl Node {
    fn new() -> Self {
        Self {
            node_id: String::new(),
            cluster: Vec::new(),
            msg_id: 0,
            counters: HashMap::new(),
        }
    }

    fn initialize(&mut self, node_id: String, node_ids: &[String]) {
        self.node_id = node_id;
        self.cluster.extend_from_slice(node_ids);
        self.counters = node_ids
            .iter()
            .map(|node_id| (node_id.clone(), 0))
            .collect();
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
        let mut message: Message = serde_json::from_str(&line)
            .context("Failed to deserialize provided input to STDIN.")?;

        let responses = message.process_received_message(&mut node);

        for response in responses {
            Node::send(&response, &mut stdout)?;
        }
    }

    Ok(())
}
