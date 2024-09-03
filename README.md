# Distributed System
This project is dedicated to learning about distributed systems through a series of challenges called [Gossip Glomers](https://fly.io/dist-sys/).

The challenges are built on top of a platform called [Maelstrom](https://github.com/jepsen-io/maelstrom), which in turn, is built on Jepsen.

## Preparations
### Getting Ready
Before starting the challenges, the following prerequisites need to be installed:
- OpenJDK
- Graphviz
- Gnuplot
- Maelstrom 0.2.3

I decided to place Maelstrom at the same directory level as my project and call it from my project directory. Therefore, I use the following command to reach it:
```
../maelstrom/maelstrom
```

External crates used in this project:
- `anyhow`
- `serde`
- `serde_json`

## Challenges
### Challenge #1: Echo
In this challenge, the goal was to receive `Init` and `Echo` requests and reply with `InitOk` and `EchoOk` according to the specification. This was essentially a warm-up exercise to ensure that Maelstrom was set up properly.

Most of my time was spent getting accustomed to the `anyhow` and `serde` crates. I decided that writing some tests to better understand `serde` was worthwhile, so `EchoServer::send` uses generics, allowing me to use `Vec<u8>` for testing and `StdoutLock` to run the server.

Maelstrom was executed with the following command to verify if the challenge was completed:
```
../maelstrom/maelstrom test -w echo --bin target/debug/echo --node-count 1 --time-limit 10

```


### Challenge #2: Unique ID Generation
This challenge was straightforward. The task was to implement a globally unique ID generation system. This was accomplished by combining `node_id`, which was received with the `Init` request, with `msg_id`, which was incremented with every sent message.

The node needed to be able to receive and process `Init` and `Generate` requests and reply with `InitOk` and `GenerateOk`.

Maelstrom was executed with the following command to verify if the challenge was completed:
```
../maelstrom/maelstrom test -w unique-ids --bin target/debug/unique_ids --time-limit 30 --rate 1000 --node-count 3 --availability total --nemesis partition
```


### Challenge #3a: Single-Node Broadcast
This set of challenges (3a - 3e) involves implementing a broadcast system that gossips messages between all nodes in the cluster. The first part focused on receiving `Init`, `Broadcast`, `Read`, and `Topology` requests and replying with `InitOk`, `BroadcastOk`, `ReadOk`, and `TopologyOk`.

Maelstrom was executed with the following command to verify if the challenge was completed:
```
../maelstrom/maelstrom test -w broadcast --bin target/debug/broadcast --node-count 1 --time-limit 20 --rate 10
```


### Challenge #3b: Multi-Node Broadcast
This part proved to be challenging. My initial solution wasn't sufficient to ensure proper message propagation.

I decided to use an `mpsc` channel and created a `Gossip` request that sent all messages to neighboring nodes at a set interval of 500ms. Although the solution wasn't efficient, it allowed me to experiment with new concepts.

Maelstrom was executed with the following command to verify if the challenge was completed:
```
../maelstrom/maelstrom test -w broadcast --bin target/debug/broadcast --node-count 5 --time-limit 20 --rate 10

```

### Challenge #3c: Fault Tolerant Broadcast
My initial solution for 3b was able to handle network partitions between nodes when they couldn't communicate for periods of time.

Maelstrom was executed with the following command to verify if the challenge was completed:
```
../maelstrom/maelstrom test -w broadcast --bin target/debug/broadcast --node-count 5 --time-limit 20 --rate 10 --nemesis partition

```

### Challenge #3d: Efficient Broadcast, Part I
Despite my best efforts, I failed this challenge by a significant margin. It proved to be more difficult than the next one.

I reduced the number of messages sent by the `Gossip` request and introduced a way for a node to track messages known by other nodes. Additionally, I added `GossipOk` to ensure that sent messages were received.

The challenge requirements:
- Messages-per-operation must be below 30
- Median latency must be below 400ms
- Maximum latency must be below 600ms

My results:
- Messages-per-operation were below 30
- Median latency was around 650ms
- Maximum latency was around 1100ms

Maelstrom was executed with the following command to verify if the challenge was completed:
```
../maelstrom/maelstrom test -w broadcast --bin target/debug/broadcast --node-count 25 --time-limit 20 --rate 100 --latency 100
```

### Challenge #3e: Efficient Broadcast, Part II
Small tweaks were enough to go below the required 20 messages per operation, with acceptable performance drops.

The challenge requirements:
- Messages-per-operation must be below 20
- Median latency must be below 1000ms
- Maximum latency must be below 2000ms

My results:
- Messages-per-operation were below 20
- Median latency was around 760ms
- Maximum latency was around 1250ms

Maelstrom was executed with the following command to verify if the challenge was completed:
```
../maelstrom/maelstrom test -w broadcast --bin target/debug/broadcast --node-count 25 --time-limit 20 --rate 100 --latency 100
```

### Challenge #4: Grow-Only Counter
The task was to implement a grow-only counter. The node needed to be able to receive and process `Init`, `Add`, and `Read` requests and reply with `InitOk`, `AddOk`, and `ReadOk`.

This was accomplished by storing every node's counter on each node. Every node increments its own counter locally. Updates are propagated with each received `Add` request via a `Sync` request, and counters are merged by taking the maximum value of each counter.

Maelstrom was executed with the following command to verify if the challenge was completed:
```
../maelstrom/maelstrom test -w g-counter --bin target/debug/g_counter --node-count 3 --rate 100 --time-limit 20 --nemesis partition
```

My solution works around 80% of the time.

Explanation:
- I decided to propagate stored counters from each node every time an `Add` request is received.
- The provided solution isn't optimal because, in cases where one of the nodes has been isolated until the very end of a test, there is no chance for all nodes to update values for stored counters.
- I tried sending an additional `Sync` request on every `Read` request as well, but it wasn't effective when a node was isolated until the very end of a test.
- I also tried a "sync back" request, but decided against keeping it as it didn't resolve the issue in all cases.
- The only viable solution seems to be the approach used in the broadcast implementation, generating `Sync` requests very frequently to ensure that, in the case of a network partition, the node can update counters quickly enough before the test ends.
- I decided to keep this faulty implementation due to its readability and because it differs from the broadcast implementation approach.
