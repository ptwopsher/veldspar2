use std::collections::HashMap;
use std::io::ErrorKind;
use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;

use renet::{ChannelConfig, ConnectionConfig, RenetServer, SendType, ServerEvent};
use tracing::warn;

pub const RELIABLE_ORDERED_CHANNEL: u8 = 0;
pub const UNRELIABLE_CHANNEL: u8 = 1;

pub const MAX_CLIENTS: usize = 32;
pub const PROTOCOL_ID: u64 = 7;
pub const PRIVATE_KEY: [u8; 32] = [0; 32];

type EventCallback = Box<dyn FnMut(u64) + Send + 'static>;

pub struct NetworkServer {
    server: RenetServer,
    socket: UdpSocket,
    client_to_addr: HashMap<u64, SocketAddr>,
    addr_to_client: HashMap<SocketAddr, u64>,
    disconnected_addrs: HashMap<u64, SocketAddr>,
    next_client_id: u64,
    connected_events: Vec<u64>,
    disconnected_events: Vec<u64>,
    connect_callback: Option<EventCallback>,
    disconnect_callback: Option<EventCallback>,
}

impl NetworkServer {
    pub fn new(port: u16) -> Self {
        let socket = UdpSocket::bind(("0.0.0.0", port))
            .unwrap_or_else(|err| panic!("failed to bind UDP socket on 0.0.0.0:{port}: {err}"));
        socket
            .set_nonblocking(true)
            .unwrap_or_else(|err| panic!("failed to enable nonblocking UDP socket: {err}"));

        let server = RenetServer::new(Self::connection_config());

        Self {
            server,
            socket,
            client_to_addr: HashMap::new(),
            addr_to_client: HashMap::new(),
            disconnected_addrs: HashMap::new(),
            next_client_id: 1,
            connected_events: Vec::new(),
            disconnected_events: Vec::new(),
            connect_callback: None,
            disconnect_callback: None,
        }
    }

    pub fn update(&mut self, dt: Duration) {
        self.connected_events.clear();
        self.disconnected_events.clear();

        self.server.update(dt);
        self.process_incoming_packets();

        for client_id in self.server.disconnections_id() {
            self.server.remove_connection(client_id);
        }

        self.process_server_events();
        self.flush_outgoing_packets();
    }

    pub fn send_to(&mut self, client_id: u64, channel: u8, data: Vec<u8>) {
        self.server.send_message(client_id, channel, data);
    }

    pub fn receive(&mut self, client_id: u64, channel: u8) -> Option<Vec<u8>> {
        self.server
            .receive_message(client_id, channel)
            .map(|bytes| bytes.to_vec())
    }

    pub fn connected_clients(&self) -> Vec<u64> {
        self.server.clients_id()
    }

    pub fn disconnect(&mut self, client_id: u64) {
        self.server.disconnect(client_id);
    }

    pub fn client_addr(&self, client_id: u64) -> Option<SocketAddr> {
        self.client_to_addr.get(&client_id).copied()
    }

    pub fn take_connected(&mut self) -> Vec<u64> {
        std::mem::take(&mut self.connected_events)
    }

    pub fn take_disconnected(&mut self) -> Vec<u64> {
        std::mem::take(&mut self.disconnected_events)
    }

    pub fn take_disconnected_addr(&mut self, client_id: u64) -> Option<SocketAddr> {
        self.disconnected_addrs.remove(&client_id)
    }

    pub fn on_connect<F>(&mut self, callback: F)
    where
        F: FnMut(u64) + Send + 'static,
    {
        self.connect_callback = Some(Box::new(callback));
    }

    pub fn on_disconnect<F>(&mut self, callback: F)
    where
        F: FnMut(u64) + Send + 'static,
    {
        self.disconnect_callback = Some(Box::new(callback));
    }

    fn connection_config() -> ConnectionConfig {
        const CHANNEL_MEMORY_BYTES: usize = 32 * 1024 * 1024;
        let channels = vec![
            ChannelConfig {
                channel_id: RELIABLE_ORDERED_CHANNEL,
                max_memory_usage_bytes: CHANNEL_MEMORY_BYTES,
                send_type: SendType::ReliableOrdered {
                    resend_time: Duration::from_millis(250),
                },
            },
            ChannelConfig {
                channel_id: UNRELIABLE_CHANNEL,
                max_memory_usage_bytes: CHANNEL_MEMORY_BYTES,
                send_type: SendType::Unreliable,
            },
        ];

        ConnectionConfig {
            available_bytes_per_tick: 200_000,
            server_channels_config: channels.clone(),
            client_channels_config: channels,
        }
    }

    fn process_incoming_packets(&mut self) {
        let mut packet_buffer = [0u8; 65_535];
        loop {
            match self.socket.recv_from(&mut packet_buffer) {
                Ok((bytes_received, from_addr)) => {
                    let is_new_client = !self.addr_to_client.contains_key(&from_addr);
                    let Some(client_id) = self.resolve_client_id(from_addr) else {
                        continue;
                    };
                    if is_new_client {
                        tracing::info!("New client {client_id} connected from {from_addr}");
                    }

                    if let Err(err) = self
                        .server
                        .process_packet_from(&packet_buffer[..bytes_received], client_id)
                    {
                        warn!("failed processing packet from {from_addr} for client {client_id}: {err}");
                    }
                }
                Err(err) if err.kind() == ErrorKind::WouldBlock => break,
                Err(err) if err.kind() == ErrorKind::Interrupted => continue,
                Err(err) => {
                    warn!("UDP receive error: {err}");
                    break;
                }
            }
        }
    }

    fn process_server_events(&mut self) {
        while let Some(event) = self.server.get_event() {
            match event {
                ServerEvent::ClientConnected { client_id } => {
                    self.connected_events.push(client_id);
                    if let Some(callback) = &mut self.connect_callback {
                        callback(client_id);
                    }
                }
                ServerEvent::ClientDisconnected { client_id, .. } => {
                    if let Some(addr) = self.client_to_addr.remove(&client_id) {
                        self.addr_to_client.remove(&addr);
                        self.disconnected_addrs.insert(client_id, addr);
                    }
                    self.disconnected_events.push(client_id);
                    if let Some(callback) = &mut self.disconnect_callback {
                        callback(client_id);
                    }
                }
            }
        }
    }

    fn flush_outgoing_packets(&mut self) {
        for client_id in self.server.clients_id() {
            let Some(addr) = self.client_to_addr.get(&client_id).copied() else {
                continue;
            };

            let packets = match self.server.get_packets_to_send(client_id) {
                Ok(packets) => packets,
                Err(_) => continue,
            };

            for packet in packets {
                if let Err(err) = self.socket.send_to(&packet, addr) {
                    if err.kind() != ErrorKind::WouldBlock && err.kind() != ErrorKind::Interrupted
                    {
                        warn!("failed sending packet to client {client_id} ({addr}): {err}");
                    }
                }
            }
        }
    }

    fn resolve_client_id(&mut self, addr: SocketAddr) -> Option<u64> {
        if let Some(&client_id) = self.addr_to_client.get(&addr) {
            return Some(client_id);
        }

        if self.client_to_addr.len() >= MAX_CLIENTS {
            return None;
        }

        let client_id = self.next_client_id;
        self.next_client_id = self.next_client_id.saturating_add(1);

        self.addr_to_client.insert(addr, client_id);
        self.client_to_addr.insert(client_id, addr);
        self.server.add_connection(client_id);

        Some(client_id)
    }
}
