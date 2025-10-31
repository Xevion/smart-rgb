// Platform-specific imports for native server
use crate::registry::ServerRegistry;
use anyhow::Result;
use borders_core::networking::{NetMessage, SourcedIntent};
use flume::Sender;
use rkyv::{Archived, api::deserialize_using, de::Pool};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{Instrument, error, info, instrument, warn};
use web_transport::quinn::{RecvStream, SendStream};

type ArchivedNetMessage = Archived<NetMessage>;

/// Handle a single client connection over WebTransport
#[instrument(skip_all)]
pub async fn handle_client_connection(mut send_stream: SendStream, mut recv_stream: RecvStream, intent_tx: Sender<SourcedIntent>, registry: Arc<RwLock<ServerRegistry>>) -> Result<()> {
    info!("New client connected, starting message handling");

    // Create a per-client channel for receiving broadcast messages
    let (client_tx, client_rx) = flume::unbounded::<NetMessage>();

    // Register this client with the server registry and get assigned player ID
    let player_id = { registry.write().await.add_client(client_tx) };
    info!(player_id = %player_id, "Client registered");

    // Send initial server config
    let server_config = NetMessage::ServerConfig { nation_id: player_id };
    let config_bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&server_config)?;
    let len_bytes = (config_bytes.len() as u64).to_be_bytes();

    // Send length prefix
    let mut written = 0;
    while written < len_bytes.len() {
        let bytes_written = send_stream.write(&len_bytes[written..]).await?;
        written += bytes_written;
    }

    // Send config bytes
    let mut written = 0;
    while written < config_bytes.len() {
        let bytes_written = send_stream.write(&config_bytes[written..]).await?;
        written += bytes_written;
    }

    // Spawn task to handle incoming intents from this client
    let intent_tx_clone = intent_tx.clone();

    tokio::spawn(
        async move {
            loop {
                // Read length prefix (8 bytes)
                let mut len_bytes = [0u8; 8];
                let mut read_so_far = 0;
                while read_so_far < 8 {
                    let buf = &mut len_bytes[read_so_far..];
                    if let Ok(maybe_size) = recv_stream.read(buf).await {
                        if let Some(size) = maybe_size {
                            read_so_far += size;
                        } else {
                            error!("Stream closed before reading length prefix");
                            break;
                        }
                    } else {
                        error!("Stream error reading length prefix");
                        break;
                    }
                }
                if read_so_far < 8 {
                    break;
                }
                let len = u64::from_be_bytes(len_bytes) as usize;

                // Read message data
                let mut message_bytes = vec![0u8; len];
                let mut read_so_far = 0;
                while read_so_far < len {
                    let buf = &mut message_bytes[read_so_far..];
                    if let Ok(maybe_size) = recv_stream.read(buf).await {
                        if let Some(size) = maybe_size {
                            read_so_far += size;
                        } else {
                            error!("Stream closed before reading full message");
                            break;
                        }
                    } else {
                        error!("Stream error reading message data");
                        break;
                    }
                }
                if read_so_far < len {
                    break;
                }

                // Decode message
                let archived = unsafe { rkyv::access_unchecked::<ArchivedNetMessage>(&message_bytes) };
                match deserialize_using::<_, _, rkyv::rancor::Error>(archived, &mut Pool::new()) {
                    Ok(net_message) => match net_message {
                        NetMessage::Intent { id, intent } => {
                            // Wrap intent with authenticated source player_id
                            let sourced_intent = SourcedIntent { source: player_id, intent_id: id, intent };
                            if let Err(e) = intent_tx_clone.send(sourced_intent) {
                                error!(error = %e, "Failed to forward intent");
                                break;
                            }
                        }
                        _ => warn!("Received unexpected message type from client"),
                    },
                    Err(e) => {
                        error!(error = %e, "Failed to deserialize message");
                        break;
                    }
                }
            }
            info!("Client intent receiver task ended");
        }
        .instrument(tracing::trace_span!(
            "client_recv_loop",
            player_id = %player_id
        )),
    );

    // Handle outgoing messages to this client
    let registry_clone = registry.clone();
    tokio::spawn(
        async move {
            while let Ok(message) = client_rx.recv_async().await {
                match rkyv::to_bytes::<rkyv::rancor::Error>(&message) {
                    Ok(message_bytes) => {
                        let len_bytes = (message_bytes.len() as u64).to_be_bytes();

                        // Send length prefix
                        let mut written = 0;
                        while written < len_bytes.len() {
                            match send_stream.write(&len_bytes[written..]).await {
                                Ok(bytes_written) => written += bytes_written,
                                Err(e) => {
                                    error!(
                                        player_id = %player_id,
                                        error = %e,
                                        "Failed to send length prefix"
                                    );
                                    break;
                                }
                            }
                        }

                        // Send message bytes
                        let mut written = 0;
                        while written < message_bytes.len() {
                            match send_stream.write(&message_bytes[written..]).await {
                                Ok(bytes_written) => written += bytes_written,
                                Err(e) => {
                                    error!(player_id = %player_id, error = %e, "Failed to send message");
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!(player_id = %player_id, error = %e, "Failed to encode message");
                        break;
                    }
                }
            }
            // Remove client from registry when sender task ends
            info!(
                player_id = %player_id,
                "Client message sender task ended, removing from registry"
            );
            registry_clone.write().await.remove_client(player_id);
        }
        .instrument(tracing::trace_span!("client_send_loop", player_id = %player_id)),
    );

    info!(
        player_id = %player_id,
        "Client connection handler setup complete"
    );
    Ok(())
}

/// Start the WebTransport server and accept connections
#[instrument(skip_all, fields(bind_address = %bind_address))]
pub async fn start_server(bind_address: &str, intent_tx: Sender<SourcedIntent>, registry: Arc<RwLock<ServerRegistry>>) -> Result<()> {
    use web_transport::quinn::ServerBuilder;

    info!("Starting WebTransport server");

    // Load development certificate and key
    let cert_path = "dev-cert.pem";
    let key_path = "dev-key.pem";

    let cert_data = std::fs::read(cert_path).map_err(|e| anyhow::anyhow!("Failed to read certificate file {}: {}", cert_path, e))?;
    let key_data = std::fs::read(key_path).map_err(|e| anyhow::anyhow!("Failed to read key file {}: {}", key_path, e))?;

    // Parse certificate and key
    let certs = rustls_pemfile::certs(&mut &cert_data[..]).collect::<Result<Vec<_>, _>>().map_err(|e| anyhow::anyhow!("Failed to parse certificate: {}", e))?;

    let key = rustls_pemfile::private_key(&mut &key_data[..]).map_err(|e| anyhow::anyhow!("Failed to parse private key: {}", e))?.ok_or_else(|| anyhow::anyhow!("No private key found"))?;

    let mut server = ServerBuilder::new().with_addr(bind_address.parse()?).with_certificate(certs, key)?;
    info!("WebTransport server listening for connections");

    loop {
        match server.accept().await {
            Some(connection) => {
                info!("New client connected");

                let intent_tx_clone = intent_tx.clone();
                let registry_clone = registry.clone();

                let session = connection.ok().await?;

                tokio::spawn(async move {
                    // Accept bidirectional stream from client
                    match session.accept_bi().await {
                        Ok((send_stream, recv_stream)) => {
                            if let Err(e) = handle_client_connection(send_stream, recv_stream, intent_tx_clone, registry_clone).await {
                                error!(error = %e, "Error handling client connection");
                            }
                        }
                        Err(e) => {
                            error!(error = %e, "Failed to accept bidirectional stream from client");
                        }
                    }
                });
            }
            None => {
                error!("Failed to accept connection");
            }
        }
    }
}
