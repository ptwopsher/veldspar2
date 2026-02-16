use std::collections::{HashMap, HashSet};
use std::io::{self, BufRead};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use glam::Vec3;
use tracing::{debug, info, warn};

use veldspar_shared::coords::{world_to_chunk, ChunkPos};
use veldspar_shared::fluid::WaterChange;
use veldspar_shared::protocol::{self, C2S, S2C};

use crate::net::{NetworkServer, RELIABLE_ORDERED_CHANNEL, UNRELIABLE_CHANNEL};
use crate::player::PlayerState;
use crate::world::ServerWorld;
use crate::commands::{self, Command};

const TICK_RATE: u32 = 20;
const TICK_DURATION: Duration = Duration::from_millis(1000 / TICK_RATE as u64);
const TIME_SYNC_INTERVAL_TICKS: u64 = (TICK_RATE as u64) * 5;
const MAX_BLOCK_EDIT_REACH: f32 = 6.0;
const MAX_BLOCK_EDIT_REACH_SQ: f32 = MAX_BLOCK_EDIT_REACH * MAX_BLOCK_EDIT_REACH;
const MAX_PLAYER_MOVE_PER_TICK: f32 = 10.0;
const CHUNK_STREAM_HORIZONTAL_RADIUS: i32 = 12;
const CHUNK_STREAM_VERTICAL_BELOW: i32 = 2;
const CHUNK_STREAM_VERTICAL_ABOVE: i32 = 1;
const WATER_TICK_INTERVAL_TICKS: u64 = 5;

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub world_path: PathBuf,
    pub port: u16,
}

pub struct Server {
    config: ServerConfig,
    world: ServerWorld,
    network: NetworkServer,
    players: HashMap<u64, PlayerState>,
    handshaken_players: HashSet<u64>,
    received_chunks_by_player: HashMap<u64, HashSet<ChunkPos>>,
    tick: u64,
    time_of_day: f32,
    running: Arc<AtomicBool>,
    command_rx: Receiver<Command>,
}

impl Server {
    pub fn new(config: ServerConfig, running: Arc<AtomicBool>, command_rx: Receiver<Command>) -> Self {
        Self {
            network: NetworkServer::new(config.port),
            world: ServerWorld::with_persistence(&config.world_path),
            players: HashMap::new(),
            handshaken_players: HashSet::new(),
            received_chunks_by_player: HashMap::new(),
            tick: 0,
            time_of_day: 0.5,
            config,
            running,
            command_rx,
        }
    }

    pub fn run(&mut self) {
        info!(
            "Starting Veldspar server on port {} (world: {})",
            self.config.port,
            self.config.world_path.display()
        );

        while self.running.load(Ordering::SeqCst) {
            let tick_start = Instant::now();

            self.handle_console_commands();
            if !self.running.load(Ordering::SeqCst) {
                break;
            }

            self.network.update(TICK_DURATION);
            self.handle_connections();
            self.handle_disconnections();
            self.receive_messages();
            self.handle_console_commands();
            if !self.running.load(Ordering::SeqCst) {
                break;
            }
            self.world.tick();
            self.tick += 1;
            if self.tick % WATER_TICK_INTERVAL_TICKS == 0 {
                let water_changes = self.world.tick_water();
                self.broadcast_water_changes(&water_changes);
            }
            self.time_of_day = (self.time_of_day + 1.0 / (TICK_RATE as f32 * 1200.0)) % 1.0;
            if self.tick % 600 == 0 {
                self.world.save_dirty_chunks();
            }
            if self.tick % TIME_SYNC_INTERVAL_TICKS == 0 {
                self.broadcast_time_sync();
            }

            // Broadcast player states every tick
            self.broadcast_player_states();

            let elapsed = tick_start.elapsed();
            if elapsed < TICK_DURATION {
                std::thread::sleep(TICK_DURATION - elapsed);
            }
        }

        self.disconnect_all_players();
        self.network.update(Duration::from_millis(0));
        self.handle_disconnections();
        info!("Server shutting down, saving world...");
        self.world.save_dirty_chunks();
        info!("World saved. Goodbye!");
    }

    fn handle_connections(&mut self) {
        for client_id in self.network.take_connected() {
            let addr = self
                .network
                .client_addr(client_id)
                .map(|addr| addr.to_string())
                .unwrap_or_else(|| "unknown".to_string());
            info!("New client connects: addr={addr}, assigned_id={client_id}");
            info!("Player {client_id} connected, waiting for handshake");
        }
    }

    fn handle_disconnections(&mut self) {
        for client_id in self.network.take_disconnected() {
            let addr = self
                .network
                .take_disconnected_addr(client_id)
                .map(|addr| addr.to_string())
                .unwrap_or_else(|| "unknown".to_string());
            info!("Client disconnects: addr={addr}, id={client_id}");
            self.players.remove(&client_id);
            let was_handshaken = self.handshaken_players.remove(&client_id);
            self.received_chunks_by_player.remove(&client_id);
            if !was_handshaken {
                continue;
            }

            let left = S2C::PlayerLeft {
                player_id: client_id,
            };
            let encoded = protocol::encode(&left);
            for &other in &self.handshaken_players {
                self.network
                    .send_to(other, RELIABLE_ORDERED_CHANNEL, encoded.clone());
            }
        }
    }

    fn receive_messages(&mut self) {
        let client_ids: Vec<u64> = self.network.connected_clients();

        for client_id in client_ids {
            // Reliable messages (block edits, handshake, chunk requests)
            while let Some(data) = self.network.receive(client_id, RELIABLE_ORDERED_CHANNEL) {
                match protocol::decode::<C2S>(&data) {
                    Ok(msg) => self.handle_c2s(client_id, msg),
                    Err(err) => warn!("Failed to decode reliable C2S from {client_id}: {err}"),
                }
            }

            // Unreliable messages (player input)
            while let Some(data) = self.network.receive(client_id, UNRELIABLE_CHANNEL) {
                match protocol::decode::<C2S>(&data) {
                    Ok(msg) => self.handle_c2s(client_id, msg),
                    Err(err) => warn!("Failed to decode unreliable C2S from {client_id}: {err}"),
                }
            }
        }
    }

    fn handle_c2s(&mut self, client_id: u64, msg: C2S) {
        match msg {
            C2S::Handshake {
                protocol_version,
                username,
            } => {
                info!(
                    "Handshake from {client_id}: username='{username}', protocol={protocol_version}"
                );
                if protocol_version != protocol::PROTOCOL_VERSION {
                    let reject = S2C::HandshakeReject {
                        reason: format!(
                            "Unsupported protocol version {protocol_version}, expected {}",
                            protocol::PROTOCOL_VERSION
                        ),
                    };
                    self.network.send_to(
                        client_id,
                        RELIABLE_ORDERED_CHANNEL,
                        protocol::encode(&reject),
                    );
                    self.network.disconnect(client_id);
                    return;
                }

                let spawn = Vec3::new(0.0, 40.0, 0.0);
                if let Some(player) = self.players.get_mut(&client_id) {
                    player.username = username.clone();
                    player.position = spawn;
                    player.last_position_tick = self.tick;
                } else {
                    let mut player = PlayerState::new(client_id, username.clone());
                    player.position = spawn;
                    player.last_position_tick = self.tick;
                    self.players.insert(client_id, player);
                }

                let accept = S2C::HandshakeAccept {
                    player_id: client_id,
                    spawn_position: spawn,
                    world_seed: self.world.world_seed(),
                    tick_rate: TICK_RATE,
                };
                info!("Sending HandshakeAccept to {client_id} with spawn {spawn}");
                self.network
                    .send_to(client_id, RELIABLE_ORDERED_CHANNEL, protocol::encode(&accept));

                if self.handshaken_players.insert(client_id) {
                    self.received_chunks_by_player
                        .entry(client_id)
                        .or_default();
                    let joined = S2C::PlayerJoined {
                        player_id: client_id,
                        username,
                        position: spawn,
                    };
                    let encoded = protocol::encode(&joined);
                    for &other in self.players.keys() {
                        if other != client_id {
                            self.network
                                .send_to(other, RELIABLE_ORDERED_CHANNEL, encoded.clone());
                        }
                    }
                }
            }
            C2S::RequestChunks { positions } => {
                if !self.handshaken_players.contains(&client_id) {
                    warn!("Ignoring RequestChunks from non-handshaken client {client_id}");
                    return;
                }

                let first = positions.first().copied();
                info!(
                    "Received RequestChunks from {client_id}: count={}, first={first:?}",
                    positions.len()
                );
                for pos in positions {
                    let chunk = self.world.get_or_generate_chunk_owned(pos);
                    let data = protocol::encode(&chunk);
                    let chunk_payload_len = data.len();
                    let response = S2C::ChunkData {
                        pos,
                        data,
                        format_version: 1,
                    };
                    debug!(
                        "Sending ChunkData {pos:?} to {client_id} ({} bytes)",
                        chunk_payload_len
                    );
                    self.network.send_to(
                        client_id,
                        RELIABLE_ORDERED_CHANNEL,
                        protocol::encode(&response),
                    );
                    self.received_chunks_by_player
                        .entry(client_id)
                        .or_default()
                        .insert(pos);
                }

                self.send_chunk_unloads_for_player(client_id);
            }
            C2S::BlockEdit { world_pos, new_block } => {
                if !self.handshaken_players.contains(&client_id) {
                    self.send_block_edit_reject(
                        client_id,
                        world_pos,
                        "handshake required before editing blocks".to_string(),
                    );
                    return;
                }

                if !self.world.is_valid_block(new_block) {
                    self.send_block_edit_reject(
                        client_id,
                        world_pos,
                        format!("invalid block id {}", new_block.0),
                    );
                    return;
                }

                let Some(player) = self.players.get(&client_id) else {
                    self.send_block_edit_reject(
                        client_id,
                        world_pos,
                        "player state not found".to_string(),
                    );
                    return;
                };

                let block_center = Vec3::new(
                    world_pos.x as f32 + 0.5,
                    world_pos.y as f32 + 0.5,
                    world_pos.z as f32 + 0.5,
                );
                let distance_sq = player.position.distance_squared(block_center);
                if distance_sq > MAX_BLOCK_EDIT_REACH_SQ {
                    self.send_block_edit_reject(
                        client_id,
                        world_pos,
                        format!(
                            "block out of reach: distance {:.2} > max {:.2}",
                            distance_sq.sqrt(),
                            MAX_BLOCK_EDIT_REACH
                        ),
                    );
                    return;
                }

                let previous_block = self.world.get_block(world_pos);
                if previous_block != new_block {
                    self.world.set_block(world_pos, new_block);
                }

                let confirm = S2C::BlockEditConfirm {
                    world_pos,
                    block: new_block,
                };
                self.network.send_to(
                    client_id,
                    RELIABLE_ORDERED_CHANNEL,
                    protocol::encode(&confirm),
                );

                if previous_block != new_block {
                    let (chunk_pos, local_pos) = world_to_chunk(world_pos);
                    let delta = S2C::ChunkDelta {
                        pos: chunk_pos,
                        changes: vec![(local_pos, new_block)],
                    };
                    let encoded_delta = protocol::encode(&delta);
                    for recipient in self.chunk_recipients(chunk_pos) {
                        self.network.send_to(
                            recipient,
                            RELIABLE_ORDERED_CHANNEL,
                            encoded_delta.clone(),
                        );
                    }
                }
            }
            C2S::PlayerInput {
                tick: _client_tick,
                position,
                yaw,
                pitch,
                flags,
                attack_animation,
                breaking_block,
                break_progress,
                ..
            } => {
                if !self.handshaken_players.contains(&client_id) {
                    warn!("Ignoring PlayerInput from non-handshaken client {client_id}");
                    return;
                }

                if let Some(player) = self.players.get_mut(&client_id) {
                    let elapsed_ticks = self.tick.saturating_sub(player.last_position_tick).max(1);
                    let max_distance = MAX_PLAYER_MOVE_PER_TICK * elapsed_ticks as f32;
                    let delta = position - player.position;
                    let distance = delta.length();

                    if distance > max_distance {
                        warn!(
                            "Suspicious movement from {client_id}: moved {:.2} blocks in {} tick(s), max {:.2}",
                            distance,
                            elapsed_ticks,
                            max_distance
                        );
                        player.position += delta.normalize_or_zero() * max_distance;
                    } else {
                        player.position = position;
                    }

                    player.last_position_tick = self.tick;
                    player.yaw = yaw;
                    player.pitch = pitch;
                    player.flags = flags;
                    player.attack_animation = attack_animation;
                    player.breaking_block = breaking_block;
                    player.break_progress = break_progress;
                }

                self.send_chunk_unloads_for_player(client_id);
            }
            C2S::Chat { message } => {
                if !self.handshaken_players.contains(&client_id) {
                    warn!("Ignoring Chat from non-handshaken client {client_id}");
                    return;
                }

                let sender_name = self
                    .players
                    .get(&client_id)
                    .map(|p| p.username.clone())
                    .unwrap_or_default();
                let chat = S2C::Chat {
                    sender_id: client_id,
                    sender_name,
                    message,
                };
                let encoded = protocol::encode(&chat);
                for &cid in self.players.keys() {
                    self.network
                        .send_to(cid, RELIABLE_ORDERED_CHANNEL, encoded.clone());
                }
            }
            C2S::Disconnect => {
                info!("Disconnect message received from client {client_id}");
                self.network.disconnect(client_id);
            }
        }
    }

    fn handle_console_commands(&mut self) {
        while let Ok(command) = self.command_rx.try_recv() {
            self.execute_console_command(command);
        }
    }

    fn execute_console_command(&mut self, command: Command) {
        match command {
            Command::Noop => {}
            Command::Stop => self.request_shutdown("console /stop"),
            Command::List => self.log_player_list(),
            Command::Say(message) => {
                info!("[CONSOLE] /say {message}");
                self.broadcast_server_chat(message);
            }
            Command::TimeSet(value) => {
                self.time_of_day = value;
                let msg = S2C::TimeSync {
                    tick: self.tick,
                    time_of_day: self.time_of_day,
                };
                let encoded = protocol::encode(&msg);
                for &client_id in &self.handshaken_players {
                    self.network
                        .send_to(client_id, RELIABLE_ORDERED_CHANNEL, encoded.clone());
                }
                info!("[CONSOLE] time set to {}", self.time_of_day);
            }
            Command::Kick(target) => match self.resolve_player_target(&target) {
                Ok(client_id) => {
                    let name = self
                        .players
                        .get(&client_id)
                        .map(|player| player.username.clone())
                        .unwrap_or_else(|| format!("Player{client_id}"));
                    info!("[CONSOLE] kicking {name} (id {client_id})");
                    self.network.disconnect(client_id);
                }
                Err(err) => warn!("[CONSOLE] /kick failed: {err}"),
            },
            Command::Teleport { player, x, y, z } => match self.resolve_player_target(&player) {
                Ok(client_id) => {
                    if let Some(target) = self.players.get_mut(&client_id) {
                        target.position = Vec3::new(x, y, z);
                        target.last_position_tick = self.tick;
                        info!(
                            "[CONSOLE] teleported {} (id {}) to [{x}, {y}, {z}]",
                            target.username, client_id
                        );
                    }
                }
                Err(err) => warn!("[CONSOLE] /tp failed: {err}"),
            },
            Command::Help => self.log_help(),
            Command::InvalidUsage(message) => warn!("[CONSOLE] {message}"),
            Command::Unknown(input) => {
                warn!("[CONSOLE] unknown command '{input}' (try /help)")
            }
        }
    }

    fn request_shutdown(&mut self, source: &str) {
        info!("Shutdown requested via {source}");
        self.broadcast_server_chat("Server shutting down...".to_string());
        self.running.store(false, Ordering::SeqCst);
    }

    fn disconnect_all_players(&mut self) {
        let connected = self.network.connected_clients();
        if connected.is_empty() {
            return;
        }

        info!("Disconnecting {} connected player(s)", connected.len());
        for client_id in connected {
            self.network.disconnect(client_id);
        }
    }

    fn resolve_player_target(&self, target: &str) -> Result<u64, String> {
        if let Ok(client_id) = target.parse::<u64>() {
            if self.players.contains_key(&client_id) {
                return Ok(client_id);
            }
            return Err(format!("player id {client_id} is not connected"));
        }

        let mut matches = self
            .players
            .iter()
            .filter(|(_, player)| player.username.eq_ignore_ascii_case(target))
            .map(|(client_id, _)| *client_id);

        let Some(first_match) = matches.next() else {
            return Err(format!("player '{target}' is not connected"));
        };

        if matches.next().is_some() {
            return Err(format!(
                "multiple players match '{target}', use /list and kick by id"
            ));
        }

        Ok(first_match)
    }

    fn broadcast_server_chat(&mut self, message: String) {
        let chat = S2C::Chat {
            sender_id: 0,
            sender_name: "Server".to_string(),
            message,
        };
        let encoded = protocol::encode(&chat);
        for &client_id in self.players.keys() {
            self.network
                .send_to(client_id, RELIABLE_ORDERED_CHANNEL, encoded.clone());
        }
    }

    fn log_player_list(&self) {
        if self.players.is_empty() {
            info!("[CONSOLE] no connected players");
            return;
        }

        let mut players: Vec<&PlayerState> = self.players.values().collect();
        players.sort_by_key(|player| player.player_id);
        info!("[CONSOLE] connected players ({}):", players.len());
        for player in players {
            info!(
                "[CONSOLE] - {} (id: {})",
                player.username, player.player_id
            );
        }
    }

    fn log_help(&self) {
        info!("[CONSOLE] Available commands:");
        info!("[CONSOLE]   /help");
        info!("[CONSOLE]   /list");
        info!("[CONSOLE]   /say <message>");
        info!("[CONSOLE]   /time set <value 0.0-1.0>");
        info!("[CONSOLE]   /kick <player|id>");
        info!("[CONSOLE]   /tp <player|id> <x> <y> <z>");
        info!("[CONSOLE]   /stop");
    }

    fn broadcast_player_states(&mut self) {
        let states: Vec<protocol::PlayerSnapshot> = self
            .players
            .iter()
            .filter(|(player_id, _)| self.handshaken_players.contains(player_id))
            .map(|(_, p)| protocol::PlayerSnapshot {
                player_id: p.player_id,
                position: p.position,
                yaw: p.yaw,
                pitch: p.pitch,
                flags: p.flags,
                attack_animation: p.attack_animation,
                breaking_block: p.breaking_block,
                break_progress: p.break_progress,
            })
            .collect();

        let msg = S2C::PlayerStates {
            tick: self.tick,
            states,
        };
        let encoded = protocol::encode(&msg);
        for &cid in &self.handshaken_players {
            self.network
                .send_to(cid, UNRELIABLE_CHANNEL, encoded.clone());
        }
    }

    fn broadcast_time_sync(&mut self) {
        let msg = S2C::TimeSync {
            tick: self.tick,
            time_of_day: self.time_of_day,
        };
        let encoded = protocol::encode(&msg);
        for &client_id in &self.handshaken_players {
            self.network
                .send_to(client_id, RELIABLE_ORDERED_CHANNEL, encoded.clone());
        }
    }

    fn chunk_recipients(&self, chunk_pos: ChunkPos) -> Vec<u64> {
        self.received_chunks_by_player
            .iter()
            .filter_map(|(client_id, chunks)| chunks.contains(&chunk_pos).then_some(*client_id))
            .collect()
    }

    fn send_chunk_unloads_for_player(&mut self, client_id: u64) {
        let Some(player) = self.players.get(&client_id) else {
            return;
        };
        let center = world_to_chunk(player.position.floor().as_ivec3()).0;
        let desired = self.gather_stream_targets(center);
        let Some(received) = self.received_chunks_by_player.get_mut(&client_id) else {
            return;
        };

        let to_unload: Vec<ChunkPos> = received
            .iter()
            .copied()
            .filter(|pos| !desired.contains(pos))
            .collect();

        for pos in to_unload {
            self.network.send_to(
                client_id,
                RELIABLE_ORDERED_CHANNEL,
                protocol::encode(&S2C::ChunkUnload { pos }),
            );
            received.remove(&pos);
        }
    }

    fn gather_stream_targets(&self, center: ChunkPos) -> HashSet<ChunkPos> {
        let mut desired = HashSet::new();
        for y in (center.y - CHUNK_STREAM_VERTICAL_BELOW)..=(center.y + CHUNK_STREAM_VERTICAL_ABOVE)
        {
            for dz in -CHUNK_STREAM_HORIZONTAL_RADIUS..=CHUNK_STREAM_HORIZONTAL_RADIUS {
                for dx in -CHUNK_STREAM_HORIZONTAL_RADIUS..=CHUNK_STREAM_HORIZONTAL_RADIUS {
                    desired.insert(ChunkPos {
                        x: center.x + dx,
                        y,
                        z: center.z + dz,
                    });
                }
            }
        }
        desired
    }

    fn send_block_edit_reject(&mut self, client_id: u64, world_pos: glam::IVec3, reason: String) {
        let reject = S2C::BlockEditReject { world_pos, reason };
        self.network.send_to(
            client_id,
            RELIABLE_ORDERED_CHANNEL,
            protocol::encode(&reject),
        );
    }

    fn broadcast_water_changes(&mut self, changes: &[WaterChange]) {
        if changes.is_empty() {
            return;
        }

        let mut by_chunk = HashMap::<ChunkPos, Vec<_>>::new();
        for change in changes {
            let (chunk_pos, local_pos) = world_to_chunk(change.world_pos);
            by_chunk
                .entry(chunk_pos)
                .or_default()
                .push((local_pos, change.new_block));
        }

        for (chunk_pos, mut chunk_changes) in by_chunk {
            chunk_changes.sort_by_key(|(local_pos, _)| (local_pos.y, local_pos.z, local_pos.x));
            let encoded = protocol::encode(&S2C::ChunkDelta {
                pos: chunk_pos,
                changes: chunk_changes,
            });
            for recipient in self.chunk_recipients(chunk_pos) {
                self.network
                    .send_to(recipient, RELIABLE_ORDERED_CHANNEL, encoded.clone());
            }
        }
    }
}

pub fn run(config: ServerConfig, running: Arc<AtomicBool>) -> io::Result<()> {
    let (command_tx, command_rx) = mpsc::channel();
    spawn_console_command_thread(command_tx);

    let mut server = Server::new(config, running, command_rx);
    server.run();
    Ok(())
}

fn spawn_console_command_thread(command_tx: Sender<Command>) {
    std::thread::spawn(move || {
        let stdin = io::stdin();
        for line_result in stdin.lock().lines() {
            let line = match line_result {
                Ok(line) => line,
                Err(err) => {
                    warn!("Failed to read server console input: {err}");
                    break;
                }
            };

            let command = commands::parse_command(&line);
            if command_tx.send(command).is_err() {
                break;
            }
        }
    });
}
