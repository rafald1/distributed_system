use std::collections::{HashMap, HashSet};
use std::io::{BufRead, StdoutLock, Write};
use std::sync::mpsc::Sender;

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
struct Gossip {
    msg_id: u64,
    messages: HashSet<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GossipOk {
    msg_id: u64,
    in_reply_to: u64,
    messages: HashSet<u64>,
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
    Gossip(Gossip),
    GossipOk(GossipOk),
}

#[derive(Debug, Serialize, Deserialize)]
struct Message {
    src: String,
    dest: String,
    body: Body,
}

impl Message {
    fn process_received_message(&mut self, node: &mut Node, sender: Sender<Event>) -> Option<Self> {
        let build_message_from = |body: Body| -> Option<Self> {
            Some(Self {
                src: self.dest.clone(),
                dest: self.src.clone(),
                body,
            })
        };

        match &mut self.body {
            Body::Init(init_body) => {
                node.initialize(init_body.node_id.clone(), sender.clone());

                build_message_from(Body::InitOk(InitOk {
                    msg_id: node.incremented_msg_id(),
                    in_reply_to: init_body.msg_id,
                }))
            }

            Body::Broadcast(broadcast_body) => {
                node.messages.insert(broadcast_body.message);

                build_message_from(Body::BroadcastOk(BroadcastOk {
                    msg_id: node.incremented_msg_id(),
                    in_reply_to: broadcast_body.msg_id,
                }))
            }

            Body::Read(read_body) => build_message_from(Body::ReadOk(ReadOk {
                msg_id: node.incremented_msg_id(),
                in_reply_to: read_body.msg_id,
                messages: node.messages.clone(),
            })),

            Body::Topology(topology_body) => {
                if let Some(neighbours) = topology_body.topology.remove(&node.node_id) {
                    node.neighbours = neighbours;
                }

                build_message_from(Body::TopologyOk(TopologyOk {
                    msg_id: node.incremented_msg_id(),
                    in_reply_to: topology_body.msg_id,
                }))
            }

            Body::Gossip(gossip_body) => {
                node.messages.extend(&gossip_body.messages);

                build_message_from(Body::GossipOk(GossipOk {
                    msg_id: node.incremented_msg_id(),
                    in_reply_to: gossip_body.msg_id,
                    messages: gossip_body.messages.clone(),
                }))
            }

            Body::GossipOk(gossip_ok_body) => {
                node.messages_seen_by_others
                    .entry(self.src.clone())
                    .or_default()
                    .extend(&gossip_ok_body.messages);
                None
            }

            Body::InitOk(_) | Body::BroadcastOk(_) | Body::ReadOk(_) | Body::TopologyOk(_) => None,
        }
    }
}

enum Event {
    Message(Message),
    GossipRequested,
    ShutdownSignal,
}

impl Event {
    fn process_received_event(
        &mut self,
        node: &mut Node,
        sender: &Sender<Event>,
        mut output: &mut StdoutLock,
    ) -> Result<(), anyhow::Error> {
        match self {
            Event::Message(message) => {
                if let Some(reply) = message.process_received_message(node, sender.clone()) {
                    Node::send(&reply, &mut output)?;
                }
                Ok(())
            }

            Event::GossipRequested => {
                for i in 0..node.neighbours.len() {
                    let new_messages: HashSet<u64> = if let Some(seen_messages) = node
                        .messages_seen_by_others
                        .get(&node.neighbours[i].clone())
                    {
                        node.messages.difference(seen_messages).cloned().collect()
                    } else {
                        node.messages.clone()
                    };

                    let gossip = Message {
                        src: node.node_id.clone(),
                        dest: node.neighbours[i].clone(),
                        body: Body::Gossip(Gossip {
                            msg_id: node.incremented_msg_id(),
                            messages: new_messages,
                        }),
                    };
                    Node::send(&gossip, &mut output)?;
                }
                Ok(())
            }

            Event::ShutdownSignal => Ok(()),
        }
    }
}

struct Node {
    node_id: String,
    msg_id: u64,
    messages: HashSet<u64>,
    neighbours: Vec<String>,
    messages_seen_by_others: HashMap<String, HashSet<u64>>,
}

impl Node {
    fn new() -> Self {
        Self {
            node_id: String::new(),
            msg_id: 0,
            messages: HashSet::new(),
            neighbours: Vec::new(),
            messages_seen_by_others: HashMap::new(),
        }
    }

    fn initialize(&mut self, node_id: String, sender: Sender<Event>) {
        self.node_id = node_id;

        // TODO: shutdown signal
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_millis(150));
            let _ = sender.send(Event::GossipRequested);
        });
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
    let (sender, receiver) = std::sync::mpsc::channel();
    let sender_clone = sender.clone();
    let mut stdout = std::io::stdout().lock();
    let mut node = Node::new();

    let join_handle = std::thread::spawn(move || {
        let stdin = std::io::stdin().lock();
        let mut stdin = stdin.lines();

        while let Ok(line) = stdin
            .next()
            .context("Maelstrom should provide input to STDIN.")?
        {
            let msg: Message = serde_json::from_str(&line)
                .context("Failed to deserialize provided input to STDIN.")?;

            if sender_clone.send(Event::Message(msg)).is_err() {
                return Ok::<_, anyhow::Error>(());
            }
        }
        Ok(())
    });

    for mut event in receiver {
        event.process_received_event(&mut node, &sender, &mut stdout)?
    }

    sender.send(Event::ShutdownSignal)?;

    join_handle
        .join()
        .map_err(|e| anyhow::anyhow!("Thread panicked: {:?}", e))??;

    Ok(())
}
