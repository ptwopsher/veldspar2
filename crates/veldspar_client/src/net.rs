use std::io::ErrorKind;
use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;

use renet::{ChannelConfig, ConnectionConfig, RenetClient, SendType};
use tracing::warn;

use veldspar_shared::protocol::{self, C2S, S2C};

pub const RELIABLE_ORDERED_CHANNEL: u8 = 0;
pub const UNRELIABLE_CHANNEL: u8 = 1;

pub struct ClientNet {
    client: RenetClient,
    socket: UdpSocket,
    server_addr: SocketAddr,
    connected: bool,
    client_id: Option<u64>,
}

impl ClientNet {
    pub fn new(server_addr: SocketAddr) -> Self {
        let socket = UdpSocket::bind(("0.0.0.0", 0))
            .unwrap_or_else(|err| panic!("failed to bind UDP client socket on 0.0.0.0:0: {err}"));
        socket
            .set_nonblocking(true)
            .unwrap_or_else(|err| panic!("failed to enable nonblocking UDP client socket: {err}"));

        Self {
            client: RenetClient::new(Self::connection_config()),
            socket,
            server_addr,
            connected: false,
            client_id: None,
        }
    }

    pub fn connect(&mut self, username: &str) {
        self.client.set_connected();
        self.connected = true;
        self.client_id = None;
        tracing::info!("Connecting to server at {}", self.server_addr);

        let handshake = C2S::Handshake {
            protocol_version: protocol::PROTOCOL_VERSION,
            username: username.to_owned(),
        };
        tracing::info!("Sending handshake as '{username}'");
        self.send_reliable(&handshake);
    }

    pub fn update(&mut self, dt: Duration) {
        self.client.update(dt);

        let mut packet_buffer = [0u8; 65_535];
        loop {
            match self.socket.recv_from(&mut packet_buffer) {
                Ok((bytes_received, from_addr)) => {
                    if from_addr == self.server_addr {
                        self.client.process_packet(&packet_buffer[..bytes_received]);
                    }
                }
                Err(err) if err.kind() == ErrorKind::WouldBlock => break,
                Err(err) if err.kind() == ErrorKind::Interrupted => continue,
                Err(err) => {
                    warn!("UDP receive error from server {}: {err}", self.server_addr);
                    break;
                }
            }
        }

        for packet in self.client.get_packets_to_send() {
            if let Err(err) = self.socket.send_to(&packet, self.server_addr) {
                if err.kind() != ErrorKind::WouldBlock && err.kind() != ErrorKind::Interrupted {
                    warn!("failed sending packet to server {}: {err}", self.server_addr);
                }
            }
        }

        if self.connected && self.client.is_disconnected() {
            tracing::warn!(
                "Disconnected from server: {:?}",
                self.client.disconnect_reason()
            );
            self.connected = false;
            self.client_id = None;
        }
    }

    pub fn send_reliable(&mut self, msg: &C2S) {
        let encoded = protocol::encode(msg);
        self.client.send_message(RELIABLE_ORDERED_CHANNEL, encoded);
    }

    pub fn send_unreliable(&mut self, msg: &C2S) {
        let encoded = protocol::encode(msg);
        self.client.send_message(UNRELIABLE_CHANNEL, encoded);
    }

    pub fn receive_reliable(&mut self) -> Vec<S2C> {
        self.receive_channel(RELIABLE_ORDERED_CHANNEL)
    }

    pub fn receive_unreliable(&mut self) -> Vec<S2C> {
        self.receive_channel(UNRELIABLE_CHANNEL)
    }

    pub fn is_connected(&self) -> bool {
        self.connected && self.client.is_connected()
    }

    pub fn disconnect(&mut self) {
        if self.connected {
            self.send_reliable(&C2S::Disconnect);
            for packet in self.client.get_packets_to_send() {
                if let Err(err) = self.socket.send_to(&packet, self.server_addr) {
                    if err.kind() != ErrorKind::WouldBlock && err.kind() != ErrorKind::Interrupted
                    {
                        warn!("failed sending disconnect packet to {}: {err}", self.server_addr);
                    }
                }
            }
        }

        self.client.disconnect();
        self.connected = false;
        self.client_id = None;
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

    fn receive_channel(&mut self, channel: u8) -> Vec<S2C> {
        let mut messages = Vec::new();
        while let Some(data) = self.client.receive_message(channel) {
            match protocol::decode::<S2C>(&data) {
                Ok(msg) => {
                    if let S2C::HandshakeAccept { player_id, .. } = &msg {
                        self.client_id = Some(*player_id);
                    }
                    messages.push(msg);
                }
                Err(err) => warn!("failed to decode S2C message on channel {channel}: {err}"),
            }
        }
        messages
    }
}
