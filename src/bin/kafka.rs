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

    Send {
        msg_id: u64,
        key: String,
        msg: u64,
    },

    SendOk {
        msg_id: u64,
        in_reply_to: u64,
        offset: u64,
    },

    Poll {
        msg_id: u64,
        offsets: HashMap<String, u64>,
    },

    PollOk {
        msg_id: u64,
        in_reply_to: u64,
        msgs: HashMap<String, Vec<[u64; 2]>>,
    },

    CommitOffsets {
        msg_id: u64,
        offsets: HashMap<String, u64>,
    },

    CommitOffsetsOk {
        msg_id: u64,
        in_reply_to: u64,
    },

    ListCommittedOffsets {
        msg_id: u64,
        keys: Vec<String>,
    },

    ListCommittedOffsetsOk {
        msg_id: u64,
        in_reply_to: u64,
        offsets: HashMap<String, u64>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct Message {
    src: String,
    dest: String,
    body: Body,
}

impl Message {
    fn process_received_message(&mut self, node: &mut Node) -> Option<Self> {
        let build_message_from = |body: Body| -> Option<Self> {
            Some(Self {
                src: self.dest.clone(),
                dest: self.src.clone(),
                body,
            })
        };

        match &mut self.body {
            Body::Init {
                msg_id, node_id, ..
            } => {
                node.initialize(node_id.clone());

                build_message_from(Body::InitOk {
                    msg_id: node.incremented_msg_id(),
                    in_reply_to: *msg_id,
                })
            }

            Body::Send { msg_id, key, msg } => {
                let offset = node.logs.get(key).map_or(0, |log| log.len()) as u64;
                node.logs.entry(key.clone()).or_default().push(*msg);

                build_message_from(Body::SendOk {
                    msg_id: node.incremented_msg_id(),
                    in_reply_to: *msg_id,
                    offset,
                })
            }

            Body::Poll { msg_id, offsets } => {
                let mut msgs: HashMap<String, Vec<[u64; 2]>> = HashMap::new();

                for (log_id, offset_from) in offsets {
                    if let Some(log) = node.logs.get(log_id) {
                        let new_messages: Vec<[u64; 2]> = log
                            .iter()
                            .enumerate()
                            .filter(|(offset, _)| *offset as u64 >= *offset_from)
                            .map(|(offset, msg)| [offset as u64, *msg])
                            .collect();

                        msgs.insert(log_id.clone(), new_messages);
                    }
                }

                build_message_from(Body::PollOk {
                    msg_id: node.incremented_msg_id(),
                    in_reply_to: *msg_id,
                    msgs,
                })
            }

            Body::CommitOffsets { msg_id, offsets } => {
                for (key, value) in offsets {
                    node.offsets.insert(key.clone(), *value);
                }

                build_message_from(Body::CommitOffsetsOk {
                    msg_id: node.incremented_msg_id(),
                    in_reply_to: *msg_id,
                })
            }

            Body::ListCommittedOffsets { msg_id, keys } => {
                let offsets: HashMap<String, u64> = node
                    .offsets
                    .iter()
                    .filter(|(log_id, _)| keys.contains(log_id))
                    .map(|(key, offset)| (key.clone(), *offset))
                    .collect();

                build_message_from(Body::ListCommittedOffsetsOk {
                    msg_id: node.incremented_msg_id(),
                    in_reply_to: *msg_id,
                    offsets,
                })
            }

            Body::InitOk { .. }
            | Body::SendOk { .. }
            | Body::PollOk { .. }
            | Body::CommitOffsetsOk { .. }
            | Body::ListCommittedOffsetsOk { .. } => None,
        }
    }
}

struct Node {
    node_id: String,
    msg_id: u64,
    logs: HashMap<String, Vec<u64>>,
    offsets: HashMap<String, u64>,
}

impl Node {
    fn new() -> Self {
        Self {
            node_id: String::new(),
            msg_id: 0,
            logs: HashMap::new(),
            offsets: HashMap::new(),
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
        let mut message: Message = serde_json::from_str(&line)
            .context("Failed to deserialize provided input to STDIN.")?;

        if let Some(reply) = message.process_received_message(&mut node) {
            Node::send(&reply, &mut stdout)?;
        } else {
            eprintln!("No reply was prepared for message: {:?}", message);
        };
    }

    Ok(())
}
