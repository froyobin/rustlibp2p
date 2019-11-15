// Copyright 2018 Parity Technologies (UK) Ltd.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the "Software"),
// to deal in the Software without restriction, including without limitation
// the rights to use, copy, modify, merge, publish, distribute, sublicense,
// and/or sell copies of the Software, and to permit persons to whom the
// Software is furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

//! Demonstrates how to perform Kademlia queries on the IPFS network.
//!
//! You can pass as parameter a base58 peer ID to search for. If you don't pass any parameter, a
//! peer ID will be generated randomly.

use futures::prelude::*;
use libp2p::kad::record::store::MemoryStore;
use libp2p::kad::{GetClosestPeersError, Kademlia, KademliaConfig, KademliaEvent};
use libp2p::{build_development_transport, identity, PeerId, Swarm};
use std::env;
use std::time::Duration;

fn main() {
    env_logger::init();

    // Create a random key for ourselves.
    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    // Set up a an encrypted DNS-enabled TCP Transport over the Mplex protocol
    let transport = build_development_transport(local_key.clone());

    // Create a swarm to manage peers and events.
    let mut swarm = {
        // Create a Kademlia behaviour.
        let mut cfg = KademliaConfig::default();
        cfg.set_query_timeout(Duration::from_secs(5 * 60));
        let store = MemoryStore::new(local_peer_id.clone());
        let mut behaviour = Kademlia::with_config(local_peer_id.clone(), store, cfg);

        // The only address that currently works.
        behaviour.add_address(
            &"Qmdf7FqEfayedTQgbxe6geqmq6QErR4GrokHABru8K5CGL"
                .parse()
                .unwrap(),
            "/ip4/127.0.0.1/tcp/5433".parse().unwrap(),
        );
        behaviour.bootstrap();

        Swarm::new(transport, behaviour, local_peer_id)
    };

    // Order Kademlia to search for a peer.
    let to_search: PeerId = if let Some(peer_id) = env::args().nth(1) {
        peer_id.parse().expect("Failed to parse peer ID to find")
    } else {
        identity::Keypair::generate_ed25519().public().into()
    };
    println!("------------>{:?}", local_key.public().into_peer_id());
    println!(
        "Searching for the closest peers to {:?}",
        local_key.public().into_peer_id()
    );
    swarm.get_closest_peers(to_search);
    libp2p::Swarm::listen_on(&mut swarm, "/ip4/127.0.0.1/tcp/5533".parse().unwrap()).unwrap();
    // Kick it off!
    tokio::run(futures::future::poll_fn(move || {
        loop {
            match swarm.poll().expect("Error while polling swarm") {
                Async::Ready(Some(KademliaEvent::GetClosestPeersResult(res))) => {
                    match res {
                        Ok(ok) => {
                            if !ok.peers.is_empty() {
                                println!("Query finished with closest peers: {:#?}", ok.peers);
                                return Ok(Async::Ready(()));
                            } else {
                                // The example is considered failed as there
                                // should always be at least 1 reachable peer.
                                println!("Query finished with no closest peers.");
                            }
                        }
                        Err(GetClosestPeersError::Timeout { peers, .. }) => {
                            if !peers.is_empty() {
                                println!("Query timed out with closest peers: {:#?}", peers);
                                return Ok(Async::Ready(()));
                            } else {
                                // The example is considered failed as there
                                // should always be at least 1 reachable peer.
                                panic!("Query timed out with no closest peers.");
                            }
                        }
                    }
                }
                Async::Ready(Some(_)) => {}
                Async::Ready(None) | Async::NotReady => break,
            }
        }

        Ok(Async::NotReady)
    }));
}