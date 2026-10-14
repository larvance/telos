//! Integration tests for singleplayer handshake and chunk streaming flow.

#![allow(clippy::too_many_lines)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use telos_content::FrozenRegistries;
use telos_core::profile::PlayerCredentials;
use telos_core::time::FixedTimestep;
use telos_net::{Connection, Lane, MemoryConnection, Payload};
use telos_protocol::bounded::BoundedString;
use telos_protocol::messages::{
    AuthMode, C2sClientSettings, C2sConfigAck, C2sHello, C2sLoginProof, C2sLoginStart, C2sMessage,
    ConnectionPhase, S2cMessage,
};
use telos_server::{Server, ServerConfig, WorldConfig};

#[test]
fn test_singleplayer_startup_flow() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let world_dir = temp_dir.path().to_path_buf();
    let registries = Arc::new(FrozenRegistries::new_default());
    let creds = PlayerCredentials::new("Player").expect("creds");

    let world_cfg = WorldConfig {
        name: "overworld".to_string(),
        seed: 42,
        generator: telos_worldgen::GeneratorKind::Standard,
        save_directory: Some(world_dir.clone()),
        ..Default::default()
    };

    let config = ServerConfig {
        tps: 20,
        view_distance: 4,
        vertical_view_distance: 4,
        simulation_distance: 4,
        chunks_per_tick_per_player: 16,
        save_directory: Some(world_dir),
        worlds: vec![world_cfg],
        enforce_tofu: false,
        ..Default::default()
    };

    let (server_conn, client_conn) = MemoryConnection::pair_default();
    let server_conn: Box<dyn Connection<S2cMessage, C2sMessage>> = Box::new(server_conn);

    let mut server = Server::with_registries(42, config, registries);
    server.add_connection(server_conn);

    // Client initiates handshake with Hello
    let _ = client_conn.send(
        Lane::Control,
        Payload::Msg(C2sMessage::Hello(C2sHello {
            protocol: 1,
            build: BoundedString::new("0.1.0").unwrap(),
            features: 0,
        })),
    );

    let mut client_phase = ConnectionPhase::Hello;
    let mut received_join = None;
    let mut received_chunks = 0;
    let mut received_uniform_chunks = 0;

    for _tick in 0..20 {
        server.tick();

        while let Ok(Some(incoming)) = client_conn.try_recv() {
            let Some(msg) = incoming.into_msg() else {
                continue;
            };

            match client_phase {
                ConnectionPhase::Hello => {
                    if let S2cMessage::HelloReply(_) = msg {
                        let login = C2sMessage::LoginStart(C2sLoginStart {
                            username: BoundedString::new(creds.username.clone()).unwrap(),
                            mode: AuthMode::Keyed,
                        });
                        let _ = client_conn.send(Lane::Control, Payload::Msg(login));
                        client_phase = ConnectionPhase::Login;
                    }
                }
                ConnectionPhase::Login => match msg {
                    S2cMessage::LoginChallenge(challenge) => {
                        let mut payload = Vec::with_capacity(15 + 32 + creds.username.len());
                        payload.extend_from_slice(b"telos-login-v1:");
                        payload.extend_from_slice(&challenge.challenge_nonce);
                        payload.extend_from_slice(creds.username.as_bytes());

                        let sig = creds.keypair().unwrap().sign(&payload);
                        let proof = C2sMessage::LoginProof(C2sLoginProof {
                            public_key: creds.public_key(),
                            signature: sig.to_bytes(),
                            certificate_data: None,
                        });
                        let _ = client_conn.send(Lane::Control, Payload::Msg(proof));
                    }
                    S2cMessage::LoginSuccess(_) => {
                        client_phase = ConnectionPhase::Config;
                    }
                    S2cMessage::Disconnect(d) => {
                        panic!("Disconnected in Login phase: {:?}", d.message.as_str());
                    }
                    _ => {}
                },
                ConnectionPhase::Config => match msg {
                    S2cMessage::ConfigDone(_) => {
                        let settings = C2sMessage::ClientSettings(C2sClientSettings {
                            view_distance: 4,
                            simulation_distance: 4,
                            locale: BoundedString::new("en_US").unwrap(),
                        });
                        let _ = client_conn.send(Lane::Control, Payload::Msg(settings));
                        let ack = C2sMessage::ConfigAck(C2sConfigAck);
                        let _ = client_conn.send(Lane::Control, Payload::Msg(ack));
                    }
                    S2cMessage::JoinGame(join) => {
                        received_join = Some(join);
                        client_phase = ConnectionPhase::Play;
                    }
                    S2cMessage::Disconnect(d) => {
                        panic!("Disconnected in Config phase: {:?}", d.message.as_str());
                    }
                    _ => {}
                },
                ConnectionPhase::Play => match msg {
                    S2cMessage::UniformChunk(_) => {
                        received_uniform_chunks += 1;
                    }
                    S2cMessage::ChunkData(_) => {
                        received_chunks += 1;
                    }
                    S2cMessage::Disconnect(d) => {
                        panic!("Disconnected in Play phase: {:?}", d.message.as_str());
                    }
                    _ => {}
                },
            }
        }
    }

    assert_eq!(client_phase, ConnectionPhase::Play);
    assert!(received_join.is_some(), "Client must receive JoinGame");
    assert!(
        received_chunks + received_uniform_chunks > 0,
        "Client must receive chunks in Play phase"
    );
}

#[test]
fn test_singleplayer_with_mismatched_cached_key() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let world_dir = temp_dir.path().to_path_buf();

    // Pre-populate usercache.json with an old / different public key
    let old_key = [0x55u8; 32];
    let usercache_content = format!(
        r#"{{"entries":{{"player":{{"username":"Player","player_uuid":"00000000-0000-0000-0000-000000000001","public_key":{:?},"first_seen_secs":1000,"last_seen_secs":1000}}}}}}"#,
        old_key.to_vec()
    );
    std::fs::write(world_dir.join("usercache.json"), usercache_content).expect("write usercache");

    let registries = Arc::new(FrozenRegistries::new_default());
    // Current player has a brand new key
    let creds = PlayerCredentials::new("Player").expect("creds");
    assert_ne!(creds.public_key(), old_key);

    let world_cfg = WorldConfig {
        name: "overworld".to_string(),
        seed: 123,
        generator: telos_worldgen::GeneratorKind::Flat,
        save_directory: Some(world_dir.clone()),
        ..Default::default()
    };

    let config = ServerConfig {
        tps: 20,
        view_distance: 4,
        vertical_view_distance: 4,
        simulation_distance: 4,
        chunks_per_tick_per_player: 16,
        save_directory: Some(world_dir),
        worlds: vec![world_cfg],
        enforce_tofu: false, // Singleplayer local mode disables TOFU lockout
        ..Default::default()
    };

    let (server_conn, client_conn) = MemoryConnection::pair_default();
    let server_conn: Box<dyn Connection<S2cMessage, C2sMessage>> = Box::new(server_conn);

    let mut server = Server::with_registries(123, config, registries);
    server.add_connection(server_conn);

    let _ = client_conn.send(
        Lane::Control,
        Payload::Msg(C2sMessage::Hello(C2sHello {
            protocol: 1,
            build: BoundedString::new("0.1.0").unwrap(),
            features: 0,
        })),
    );

    let mut client_phase = ConnectionPhase::Hello;
    let mut received_join = None;

    for _tick in 0..20 {
        server.tick();

        while let Ok(Some(incoming)) = client_conn.try_recv() {
            let Some(msg) = incoming.into_msg() else {
                continue;
            };

            match client_phase {
                ConnectionPhase::Hello => {
                    if let S2cMessage::HelloReply(_) = msg {
                        let login = C2sMessage::LoginStart(C2sLoginStart {
                            username: BoundedString::new(creds.username.clone()).unwrap(),
                            mode: AuthMode::Keyed,
                        });
                        let _ = client_conn.send(Lane::Control, Payload::Msg(login));
                        client_phase = ConnectionPhase::Login;
                    }
                }
                ConnectionPhase::Login => match msg {
                    S2cMessage::LoginChallenge(challenge) => {
                        let mut payload = Vec::with_capacity(15 + 32 + creds.username.len());
                        payload.extend_from_slice(b"telos-login-v1:");
                        payload.extend_from_slice(&challenge.challenge_nonce);
                        payload.extend_from_slice(creds.username.as_bytes());

                        let sig = creds.keypair().unwrap().sign(&payload);
                        let proof = C2sMessage::LoginProof(C2sLoginProof {
                            public_key: creds.public_key(),
                            signature: sig.to_bytes(),
                            certificate_data: None,
                        });
                        let _ = client_conn.send(Lane::Control, Payload::Msg(proof));
                    }
                    S2cMessage::LoginSuccess(_) => {
                        client_phase = ConnectionPhase::Config;
                    }
                    S2cMessage::Disconnect(d) => {
                        panic!(
                            "Should not disconnect on singleplayer TOFU mismatch: {:?}",
                            d.message.as_str()
                        );
                    }
                    _ => {}
                },
                ConnectionPhase::Config => match msg {
                    S2cMessage::ConfigDone(_) => {
                        let settings = C2sMessage::ClientSettings(C2sClientSettings {
                            view_distance: 4,
                            simulation_distance: 4,
                            locale: BoundedString::new("en_US").unwrap(),
                        });
                        let _ = client_conn.send(Lane::Control, Payload::Msg(settings));
                        let ack = C2sMessage::ConfigAck(C2sConfigAck);
                        let _ = client_conn.send(Lane::Control, Payload::Msg(ack));
                    }
                    S2cMessage::JoinGame(join) => {
                        received_join = Some(join);
                        client_phase = ConnectionPhase::Play;
                    }
                    S2cMessage::Disconnect(d) => {
                        panic!("Disconnected in Config phase: {:?}", d.message.as_str());
                    }
                    _ => {}
                },
                ConnectionPhase::Play => {}
            }
        }
    }

    assert_eq!(client_phase, ConnectionPhase::Play);
    assert!(
        received_join.is_some(),
        "Client should successfully enter Play despite old cached key"
    );
    // Verify usercache was updated with new key
    let entry = server
        .usercache
        .lookup_by_username("player")
        .expect("usercache entry");
    assert_eq!(entry.public_key, Some(creds.public_key()));
}

#[test]
fn test_threaded_singleplayer_server_flow() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let world_dir = temp_dir.path().to_path_buf();
    let registries = Arc::new(FrozenRegistries::new_default());
    let creds = PlayerCredentials::new("Player").expect("creds");

    let world_cfg = WorldConfig {
        name: "overworld".to_string(),
        seed: 999,
        generator: telos_worldgen::GeneratorKind::Flat,
        save_directory: Some(world_dir.clone()),
        ..Default::default()
    };

    let config = ServerConfig {
        tps: 20,
        view_distance: 4,
        vertical_view_distance: 4,
        simulation_distance: 4,
        chunks_per_tick_per_player: 16,
        save_directory: Some(world_dir),
        worlds: vec![world_cfg],
        enforce_tofu: false,
        ..Default::default()
    };

    let (server_conn, client_conn) = MemoryConnection::pair_default();
    let server_conn: Box<dyn Connection<S2cMessage, C2sMessage>> = Box::new(server_conn);

    let server_running = Arc::new(AtomicBool::new(true));
    let running_clone = server_running.clone();
    let s_registries = Arc::clone(&registries);

    let server_handle = std::thread::Builder::new()
        .name("test-singleplayer-server".into())
        .spawn(move || {
            let mut server = Server::with_registries(999, config, s_registries);
            server.add_connection(server_conn);

            let mut timestep = FixedTimestep::new(20);
            while running_clone.load(Ordering::Relaxed) {
                timestep.advance(|_| {
                    server.tick();
                });
                std::thread::sleep(Duration::from_millis(1));
            }
        })
        .expect("spawn server thread");

    let _ = client_conn.send(
        Lane::Control,
        Payload::Msg(C2sMessage::Hello(C2sHello {
            protocol: 1,
            build: BoundedString::new("0.1.0").unwrap(),
            features: 0,
        })),
    );

    let mut client_phase = ConnectionPhase::Hello;
    let mut received_join = None;
    let mut received_chunks = 0;

    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(3) {
        while let Ok(Some(incoming)) = client_conn.try_recv() {
            let Some(msg) = incoming.into_msg() else {
                continue;
            };

            match client_phase {
                ConnectionPhase::Hello => {
                    if let S2cMessage::HelloReply(_) = msg {
                        let login = C2sMessage::LoginStart(C2sLoginStart {
                            username: BoundedString::new(creds.username.clone()).unwrap(),
                            mode: AuthMode::Keyed,
                        });
                        let _ = client_conn.send(Lane::Control, Payload::Msg(login));
                        client_phase = ConnectionPhase::Login;
                    }
                }
                ConnectionPhase::Login => match msg {
                    S2cMessage::LoginChallenge(challenge) => {
                        let mut payload = Vec::with_capacity(15 + 32 + creds.username.len());
                        payload.extend_from_slice(b"telos-login-v1:");
                        payload.extend_from_slice(&challenge.challenge_nonce);
                        payload.extend_from_slice(creds.username.as_bytes());

                        let sig = creds.keypair().unwrap().sign(&payload);
                        let proof = C2sMessage::LoginProof(C2sLoginProof {
                            public_key: creds.public_key(),
                            signature: sig.to_bytes(),
                            certificate_data: None,
                        });
                        let _ = client_conn.send(Lane::Control, Payload::Msg(proof));
                    }
                    S2cMessage::LoginSuccess(_) => {
                        client_phase = ConnectionPhase::Config;
                    }
                    S2cMessage::Disconnect(d) => {
                        panic!("Disconnected in Login phase: {:?}", d.message.as_str());
                    }
                    _ => {}
                },
                ConnectionPhase::Config => match msg {
                    S2cMessage::ConfigDone(_) => {
                        let settings = C2sMessage::ClientSettings(C2sClientSettings {
                            view_distance: 4,
                            simulation_distance: 4,
                            locale: BoundedString::new("en_US").unwrap(),
                        });
                        let _ = client_conn.send(Lane::Control, Payload::Msg(settings));
                        let ack = C2sMessage::ConfigAck(C2sConfigAck);
                        let _ = client_conn.send(Lane::Control, Payload::Msg(ack));
                    }
                    S2cMessage::JoinGame(join) => {
                        received_join = Some(join);
                        client_phase = ConnectionPhase::Play;
                    }
                    S2cMessage::Disconnect(d) => {
                        panic!("Disconnected in Config phase: {:?}", d.message.as_str());
                    }
                    _ => {}
                },
                ConnectionPhase::Play => match msg {
                    S2cMessage::UniformChunk(_) | S2cMessage::ChunkData(_) => {
                        received_chunks += 1;
                    }
                    S2cMessage::Disconnect(d) => {
                        panic!("Disconnected in Play phase: {:?}", d.message.as_str());
                    }
                    _ => {}
                },
            }
        }

        if client_phase == ConnectionPhase::Play && received_chunks > 0 {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }

    server_running.store(false, Ordering::Relaxed);
    let _ = server_handle.join();

    assert_eq!(client_phase, ConnectionPhase::Play);
    assert!(received_join.is_some(), "Client must receive JoinGame");
    assert!(
        received_chunks > 0,
        "Client must receive chunks in Play phase"
    );
}
