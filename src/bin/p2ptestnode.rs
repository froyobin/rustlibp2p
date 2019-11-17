// Copyright 20l9 Parity Technologies (UK) Ltd.
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

//! A basic key value store demonstrating libp2p and the mDNS and Kademlia protocols.
//!
//! 1. Using two terminal windows, start two instances. If you local network
//!    allows mDNS, they will automatically connect.
//!
//! 2. Type `PUT my-key my-value` in terminal one and hit return.
//!
//! 3. Type `GET my-key` in terminal two and hit return.
//!
//! 4. Close with Ctrl-c.

use futures::prelude::*;
use libp2p::floodsub::protocol::FloodsubMessage;
use libp2p::kad::record;
use libp2p::kad::record::store::MemoryStore;
use libp2p::kad::{record::Key, Kademlia, KademliaEvent, PutRecordOk, Quorum, Record};
use libp2p::{
    build_development_transport, identity,
    mdns::{Mdns, MdnsEvent},
    swarm::NetworkBehaviourEventProcess,
    tokio_codec::{FramedRead, LinesCodec},
    tokio_io::{AsyncRead, AsyncWrite},
    NetworkBehaviour, PeerId, Swarm,
};
use std::env;
use std::thread;
use std::time::Duration;
#[derive(NetworkBehaviour)]
struct MyBehaviour<TSubstream: AsyncRead + AsyncWrite> {
    discovery: Kademlia<TSubstream, MemoryStore>,
    floodsub: libp2p::floodsub::Floodsub<TSubstream>,
}

impl<TSubstream: libp2p::tokio_io::AsyncRead + libp2p::tokio_io::AsyncWrite>
    libp2p::swarm::NetworkBehaviourEventProcess<libp2p::floodsub::FloodsubEvent>
    for MyBehaviour<TSubstream>
{
    // Called when `floodsub` produces an event.
    fn inject_event(&mut self, message: libp2p::floodsub::FloodsubEvent) {
        if let libp2p::floodsub::FloodsubEvent::Message(message) = message {
            println!(">>>>>>>>>>>>>>>>>>>>>>>>>{:?}", message);
        }
    }
}

impl<TSubstream: AsyncRead + AsyncWrite> NetworkBehaviourEventProcess<KademliaEvent>
    for MyBehaviour<TSubstream>
{
    // Called when `kademlia` produces an event.
    fn inject_event(&mut self, message: KademliaEvent) {
        // println!("{:?}", message);
        match message {
            KademliaEvent::GetRecordResult(Ok(result)) => {
                for Record { key, value, .. } in result.records {
                    println!(
                        "Got record {:?} {:?}",
                        std::str::from_utf8(key.as_ref()).unwrap(),
                        std::str::from_utf8(&value).unwrap(),
                    );
                }
            }
            KademliaEvent::GetRecordResult(Err(err)) => {
                eprintln!("Failed to get record: {:?}", err);
            }
            KademliaEvent::PutRecordResult(Ok(PutRecordOk { key })) => {
                println!(
                    "Successfully put record {:?}",
                    std::str::from_utf8(key.as_ref()).unwrap()
                );
            }
            KademliaEvent::PutRecordResult(Err(err)) => {
                eprintln!("Failed to put record: {:?}", err);
            }

            KademliaEvent::UnroutablePeer { peer, .. } => {
                self.floodsub.add_node_to_partial_view(peer);
            }

            KademliaEvent::GetClosestPeersResult(res) => {
                match res {
                    Ok(ok) => {
                        if !ok.peers.is_empty() {
                            let peers = ok.peers;
                            for i in peers.into_iter() {
                                self.floodsub.add_node_to_partial_view(i);
                            }
                        // return Ok(Async::Ready(()));
                        } else {
                            // The example is considered failed as there
                            // should always be at least 1 reachable peer.
                            // println!("Query finished with no closest peers.");
                        }
                    }
                    Err(_) => {
                        println!("error!!!!!");
                    }
                }
            }

            _ => {}
        }
    }
}

fn main() {
    env_logger::init();

    // Create a random key for ourselves.
    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    println!("------------->{:?}", local_key.public().into_peer_id());
    // Set up a an encrypted DNS-enabled TCP Transport over the Mplex protocol.
    let transport = build_development_transport(local_key);

    // We create a custom network behaviour that combines Kademlia and mDNS.
    let broadcasttopic = libp2p::floodsub::TopicBuilder::new("testtopic").build();

    // Create a swarm to manage peers and events.

    // Create a Kademlia behaviour.
    let store = MemoryStore::new(local_peer_id.clone());
    let kademlia = Kademlia::new(local_peer_id.clone(), store);

    let mut behaviour = MyBehaviour {
        discovery: kademlia,
        floodsub: libp2p::floodsub::Floodsub::new(local_peer_id.clone()),
    };
    behaviour.discovery.add_address(
        &"QmWXaBDDEZZbhJro4tNftHDZ2XuQwpoC2GUuWDDku8HZGg"
            .parse()
            .unwrap(),
        "/ip4/127.0.0.1/tcp/56930".parse().unwrap(),
    );

    behaviour.floodsub.subscribe(broadcasttopic.clone());
    behaviour
        .discovery
        .start_providing(record::Key::new(&"hello".as_bytes()));

    behaviour.discovery.bootstrap();
    let mut swarm = Swarm::new(transport, behaviour, local_peer_id.clone());

    // Read full lines from stdin.
    let stdin = tokio_stdin_stdout::stdin(0);
    let mut framed_stdin = FramedRead::new(stdin, LinesCodec::new());

    // Listen on all interfaces and whatever port the OS assigns.

    Swarm::listen_on(&mut swarm, "/ip4/0.0.0.0/tcp/0".parse().unwrap()).unwrap();
    // swarm.discovery.add_address(
    //     &local_peer_id.clone(),
    //     "/ip4/127.0.0.1/tcp/0".parse().unwrap(),
    // );

    swarm.discovery.add_address(
        &"QmWXaBDDEZZbhJro4tNftHDZ2XuQwpoC2GUuWDDku8HZGg"
            .parse()
            .unwrap(),
        "/ip4/127.0.0.1/tcp/56930".parse().unwrap(),
    );

    // Kick it off.
    let mut listening = false;
    tokio::run(futures::future::poll_fn(move || {
        loop {
            // println!("1111111111111111111111111111");
            swarm.floodsub.publish(&broadcasttopic, "hello".to_owned());
            thread::sleep(Duration::from_secs(2));
            fuck_rust(
                &mut swarm.discovery,
                identity::Keypair::generate_ed25519().public().into(),
            );
            // thread::sleep(Duration::from_secs(2));
            break;
        }

        loop {
            match swarm.poll().expect("Error while polling swarm") {
                Async::Ready(Some(_)) => {}
                Async::Ready(None) | Async::NotReady => {
                    if !listening {
                        if let Some(a) = Swarm::listeners(&swarm).next() {
                            println!("Listening on {:?}", a);
                            listening = true;
                        }
                    }
                    break;
                }
            }
            break;
        }
        // loop {
        //     println!("3333333333333333333");
        //     swarm.floodsub.publish(&broadcasttopic, "hello".to_owned());

        //     // thread::sleep(Duration::from_secs(2));
        //     break;
        // }
        Ok(Async::NotReady)
    }));
}

fn fuck_rust<TSubstream: AsyncRead + AsyncWrite>(
    discovery: &mut Kademlia<TSubstream, MemoryStore>,
    line: PeerId,
) {
    discovery.get_closest_peers(line);
}

fn handle_input_line<TSubstream: AsyncRead + AsyncWrite>(
    kademlia: &mut Kademlia<TSubstream, MemoryStore>,
    line: String,
) {
    let mut args = line.split(" ");

    match args.next() {
        Some("GET") => {
            let key = {
                match args.next() {
                    Some(key) => Key::new(&key),
                    None => {
                        eprintln!("Expected key");
                        return;
                    }
                }
            };
            kademlia.get_record(&key, Quorum::One);
        }
        Some("PUT") => {
            let key = {
                match args.next() {
                    Some(key) => Key::new(&key),
                    None => {
                        eprintln!("Expected key");
                        return;
                    }
                }
            };
            let value = {
                match args.next() {
                    Some(value) => value.as_bytes().to_vec(),
                    None => {
                        eprintln!("Expected value");
                        return;
                    }
                }
            };
            let record = Record {
                key,
                value,
                publisher: None,
                expires: None,
            };
            kademlia.put_record(record, Quorum::One);
        }
        _ => {
            eprintln!("expected GET or PUT");
        }
    }
}
