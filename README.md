## Little Dipper   🥄

This is an experimental software out of personal interest in building a censorship resistant consensus protocol inspired by (bigdipper)[https://arxiv.org/abs/2307.10185].

Little Dipper is a consensus module that augments a classic leader based consensus with a property called short term censorship resistance. It guarantees transaction inclusion in the next block even if the current and future leaders have the intent of censoring. It is especially useful for consensus with a stable leader, but also effective for rotating leader scheme.

A user who worry about tx censorship can send its transaction to multiple validators, and an honest validator includes the tx into its mini-block for some view. Validators send mini-block to the leader for the view, and only sufficient mini-blocks are collected, a leader is permitted to send its proposal. Validator will check if there sufficient is sufficient number of mini-blocks for a view when verifying the proposal.

### How to run
This repo used a slightly augmented simplex consensus that notifies the application when nullify took place. To run `little-dipper` 
```
git clone git@github.com:bx3/monorepo.git
cargo build
```

To run the consensus, see [bench/cmd.sh](https://github.com/bx3/little-dipper/blob/master/bench/cmd.sh)

### BigDipper :milky_way:

BigDipper is system that can augment a classic leader based consensus protocol with censorship resistance. This system can be designed to be either scalable or non-scalable, determining by if the total throughput is $O(nC)$, where $n$ is the number of validdtor, and $C$ is the bandwidth of each vaildator. Little Dipper 🥄, only works in the regime of $O(C)$, and hence non-scalable (or called it veritially scalable by the node hardware spec).

### Simplex

Little Dipper can augment the censorship resistance property to any classic leader based consensus, including HotStuff, Hotstuff-2, Tendermint or PBFT. Simplex is new construction of the leader based protocol that has appealing properties. 
- when leader and network are good, and block time is $2\delta$
- when leader and network are good, block confirmation time is $3\delta$

Simplex has another advantage of easier to understand than other consensus protocol. However, Simplex relies on a P2P network for synchronizing the view. On the other hand, this simplifies the pace-making. 


## System Description

- API-Server (To be implemented): a http server listening to user's request, and forward it to Chatter

- P2P-Server: an instance that connects to a p2p channel whose purpose is to transmit mini-block to the leader

- Chatter: a chatter is responsible for organizing mini-blocks for p2p network and consensus. For a leader instance, the chatter actor prepares a proposal by combining mini-blocks for a particular view; for a validator instance, the chatter actor sends mini-block to the leader for each view, and verify if sufficient mini-blocks are received when the consensus asking for verifying the mini-blocks.

### Consensus State Transition Diagram
<img width="1146" alt="Screenshot 2025-01-19 at 11 30 44 AM" src="https://github.com/user-attachments/assets/cd6ed7c8-0956-4695-9dca-6d669c8a0ec1" />

### System Diagram
<img width="924" alt="Screenshot 2025-01-19 at 11 46 29 AM" src="https://github.com/user-attachments/assets/cfeaca31-4720-4133-8f7f-3be71c221b5c" />

