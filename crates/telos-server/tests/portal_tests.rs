//! Integration tests for Nether portal ignition, contiguous deactivation,
//! 8:1 coordinate scaling, teleporter cache, and dimensional transfer.

use glam::DVec3;
use telos_core::coords::BlockPos;
use telos_core::ident::Identifier;
use telos_net::{Connection, Lane, MemoryConnection, Payload};
use telos_protocol::bounded::BoundedString;
use telos_protocol::messages::{
    AuthMode, BlockActionKind, C2sBlockAction, C2sConfigAck, C2sHello, C2sLoginStart, C2sMessage,
    S2cMessage,
};
use telos_server::{GeneratorKind, Server, ServerConfig};
use telos_sim::{GameMode, ITEM_FLINT_AND_STEEL};
use telos_voxel::state::BlockStateId;

fn complete_handshake(
    server: &mut Server,
    client_conn: &MemoryConnection<C2sMessage, S2cMessage>,
    username: &str,
) {
    // 1. Hello
    client_conn
        .send(
            Lane::Control,
            Payload::Msg(C2sMessage::Hello(C2sHello {
                protocol: 1,
                build: BoundedString::new("0.1.0").unwrap(),
                features: 0,
            })),
        )
        .expect("send Hello");
    server.tick();
    let _ = client_conn.try_recv();

    // 2. Login
    client_conn
        .send(
            Lane::Control,
            Payload::Msg(C2sMessage::LoginStart(C2sLoginStart {
                username: BoundedString::new(username).unwrap(),
                mode: AuthMode::Offline,
            })),
        )
        .expect("send LoginStart");
    server.tick();
    let _ = client_conn.try_recv();

    // 3. ConfigAck
    client_conn
        .send(
            Lane::Control,
            Payload::Msg(C2sMessage::ConfigAck(C2sConfigAck {})),
        )
        .expect("send ConfigAck");
    server.tick();

    // Drain JoinGame and initial packets
    while let Ok(Some(_)) = client_conn.try_recv() {}
}

#[test]
fn test_nether_dimension_initialization() {
    let server = Server::new(12345, ServerConfig::default());
    assert!(server.worlds().contains("nether"));

    let nether_world = server.worlds().get("nether").expect("nether world exists");
    assert_eq!(nether_world.generator_kind(), GeneratorKind::Nether);
}

#[test]
fn test_portal_frame_ignition_and_breaking() {
    let mut server = Server::new(4242, ServerConfig::default());
    let (server_conn, client_conn) = MemoryConnection::pair_default();
    let session_id = server.add_connection(Box::new(server_conn));
    complete_handshake(&mut server, &client_conn, "PortalTester");

    // Position player within reach of (10, 64, 20)
    {
        let session = server.get_session_mut(session_id).unwrap();
        session.position = DVec3::new(10.0, 64.0, 18.0);
        session.move_state.pos = session.position;
    }

    let obsidian_id = server
        .registries
        .block_registry()
        .get(&Identifier::new("telos", "obsidian").unwrap())
        .unwrap()
        .default_state();
    let portal_id = server
        .registries
        .block_registry()
        .get(&Identifier::new("telos", "nether_portal").unwrap())
        .unwrap()
        .default_state();

    // 1. Build a 4x5 obsidian frame in the Overworld:
    // Bottom-left interior is at (10, 64, 20).
    // Interior width 2 (x in 10..=11), interior height 3 (y in 64..=66), z = 20.
    let (frame_blocks, interior_blocks, _) = telos_sim::generate_portal_structure_blocks(
        BlockPos::new(10, 64, 20),
        telos_sim::PortalAxis::X,
    );

    {
        let world = server.worlds_mut().get_or_default_mut("overworld");
        for p in &frame_blocks {
            let _ = world.set_block(*p, obsidian_id);
        }
        for p in &interior_blocks {
            let _ = world.set_block(*p, BlockStateId::AIR);
        }
    }

    // 2. Right-click with Flint and Steel at (10, 64, 20)
    let action_msg = C2sMessage::BlockAction(C2sBlockAction {
        action: BlockActionKind::Place {
            state_id: BlockStateId::new(ITEM_FLINT_AND_STEEL),
            hit_face: 1, // Top of bottom sill
        },
        x: 10,
        y: 63, // Top of sill at y=63 points into y=64
        z: 20,
        sequence: 1,
        input_tick: 1,
    });
    client_conn
        .send(Lane::Control, Payload::Msg(action_msg))
        .unwrap();
    server.tick();

    // Verify all 6 interior cells are filled with nether_portal
    {
        let world = server.worlds_mut().get_or_default_mut("overworld");
        for p in &interior_blocks {
            assert_eq!(
                world.get_block(*p),
                portal_id,
                "Interior cell at {p:?} must be nether_portal"
            );
        }
    }
    assert!(
        server
            .portal_cache
            .get("overworld")
            .unwrap()
            .contains(&interior_blocks[0]),
        "Portal cache must contain activated portal blocks"
    );

    // 3. Break one obsidian frame block at (9, 64, 20)
    let break_msg = C2sMessage::BlockAction(C2sBlockAction {
        action: BlockActionKind::Break,
        x: 9,
        y: 64,
        z: 20,
        sequence: 2,
        input_tick: 2,
    });
    client_conn
        .send(Lane::Control, Payload::Msg(break_msg))
        .unwrap();
    server.tick();

    // Verify all contiguous nether_portal blocks have reverted to AIR
    {
        let world = server.worlds_mut().get_or_default_mut("overworld");
        for p in &interior_blocks {
            assert_eq!(
                world.get_block(*p),
                BlockStateId::AIR,
                "Interior cell at {p:?} must revert to AIR after frame is broken"
            );
        }
    }
}

#[test]
fn test_dimensional_transfer_roundtrip() {
    let mut server = Server::new(7777, ServerConfig::default());
    let (server_conn, client_conn) = MemoryConnection::pair_default();
    let session_id = server.add_connection(Box::new(server_conn));
    complete_handshake(&mut server, &client_conn, "DimensionTraveler");

    // Set creative mode for instant portal activation
    server.apply_game_mode(session_id, GameMode::Creative);

    let obsidian_id = server
        .registries
        .block_registry()
        .get(&Identifier::new("telos", "obsidian").unwrap())
        .unwrap()
        .default_state();
    let portal_id = server
        .registries
        .block_registry()
        .get(&Identifier::new("telos", "nether_portal").unwrap())
        .unwrap()
        .default_state();

    // Build and ignite a portal at interior (80, 64, 160)
    let (frame_blocks, interior_blocks, _) = telos_sim::generate_portal_structure_blocks(
        BlockPos::new(80, 64, 160),
        telos_sim::PortalAxis::X,
    );

    {
        let world = server.worlds_mut().get_or_default_mut("overworld");
        for p in &frame_blocks {
            let _ = world.set_block(*p, obsidian_id);
        }
        for p in &interior_blocks {
            let _ = world.set_block(*p, portal_id);
        }
        for p in &interior_blocks {
            server
                .portal_cache
                .entry("overworld".to_string())
                .or_default()
                .insert(*p);
        }
    }

    // Step player inside portal at (80.5, 64.0, 160.5)
    {
        let session = server.get_session_mut(session_id).unwrap();
        session.position = DVec3::new(80.5, 64.0, 160.5);
        session.move_state.pos = session.position;
        session.portal_timer = 0;
        session.portal_cooldown = 0;
    }

    // Tick server: in Creative, threshold is 1 tick
    server.tick();

    // Verify player is now in Nether
    let (dest_world, dest_pos) = {
        let session = server.get_session(session_id).unwrap();
        (session.world_name.clone(), session.position)
    };
    assert_eq!(dest_world, "nether");

    // Coordinates scaled 8:1 horizontally: 80.5 / 8 = ~10.0625, 160.5 / 8 = ~20.0625
    assert!((dest_pos.x - 10.5).abs() < 2.0);
    assert!((dest_pos.z - 20.5).abs() < 2.0);
    assert!(dest_pos.y >= 32.0 && dest_pos.y <= 118.0);

    // Verify a portal structure exists at destination in Nether
    {
        let nether_world = server.worlds_mut().get_or_default_mut("nether");
        let nether_pos = BlockPos::new(
            dest_pos.x.floor() as i32,
            dest_pos.y.floor() as i32,
            dest_pos.z.floor() as i32,
        );
        assert_eq!(nether_world.get_block(nether_pos), portal_id);
    }

    // Clear cooldown to test return trip
    {
        let session = server.get_session_mut(session_id).unwrap();
        session.portal_cooldown = 0;
    }

    // Tick server for return trip
    server.tick();

    let session = server.get_session(session_id).unwrap();
    assert_eq!(session.world_name, "overworld");
    // Coordinates scaled back to Overworld: ~80.5, ~160.5
    assert!((session.position.x - 80.5).abs() < 3.0);
    assert!((session.position.z - 160.5).abs() < 3.0);
}
