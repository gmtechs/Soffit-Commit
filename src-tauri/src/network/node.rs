use crate::network::{protocol::SOFFIT_ALPN, sync};
use anyhow::Result;
use iroh::{endpoint::presets, Endpoint, EndpointAddr, SecretKey};
use iroh_gossip::net::{Gossip, GOSSIP_ALPN};
use iroh_gossip::proto::TopicId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::broadcast;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum NetworkEvent {
    PeerOnline {
        node_id: String,
        display_name: String,
        endpoint_addr: String,
    },
    PeerOffline {
        node_id: String,
    },
    LockAcquired {
        file_path: String,
        peer_name: String,
    },
    LockReleased {
        file_path: String,
    },
    PermissionChanged {
        share_id: String,
        peer_id: String,
        level: String,
    },
    /// A remote share was granted to this device and a local copy folder
    /// now exists (emitted on the receiving side).
    ShareGranted {
        share_id: String,
    },
    SyncComplete {
        share_id: String,
        path: String,
    },
    SyncError {
        path: String,
        error: String,
    },
}

pub struct IrohNode {
    pub endpoint: Endpoint,
    pub gossip: Arc<Gossip>,
    pub node_id: String,
    pub event_tx: broadcast::Sender<NetworkEvent>,
    /// SQLite database location (opened per-task; rusqlite Connection is not Sync).
    pub db_path: PathBuf,
    /// Folder under which remote shares are stored: `<shared>/<share_id>/`.
    pub shared_root: PathBuf,
    /// Live peer connections (node_id → connection + that connection's sync
    /// state) so commands can reach an online peer on demand — pull-mode
    /// selective sync ("fetch on open", technical spec §12).
    pub conns: std::sync::Mutex<HashMap<String, (iroh::endpoint::Connection, Arc<sync::SharedCtx>)>>,
    /// Wakes the per-connection push loops the moment a watched share folder
    /// changes on disk (realtime sync) instead of waiting for SYNC_INTERVAL.
    pub change_notify: Arc<tokio::sync::Notify>,
}

impl IrohNode {
    pub async fn start(
        secret_key: SecretKey,
        db_path: PathBuf,
        shared_root: PathBuf,
    ) -> Result<Arc<Self>> {
        let endpoint = Endpoint::builder(presets::N0)
            .secret_key(secret_key)
            // Both ALPNs must be registered: the sync/Hello protocol and the
            // gossip protocol (locks, presence, permission changes).
            .alpns(vec![SOFFIT_ALPN.to_vec(), GOSSIP_ALPN.to_vec()])
            .bind()
            .await?;

        let node_id = endpoint.id().to_string();
        let gossip = Arc::new(Gossip::builder().spawn(endpoint.clone()));
        let (event_tx, _) = broadcast::channel(256);

        let node = Arc::new(IrohNode {
            endpoint,
            gossip,
            node_id,
            event_tx,
            db_path,
            shared_root,
            conns: std::sync::Mutex::new(HashMap::new()),
            change_notify: Arc::new(tokio::sync::Notify::new()),
        });

        let node_clone = node.clone();
        tokio::spawn(async move {
            node_clone.accept_loop().await;
        });

        // Filesystem watchers: wake the sync engine the moment a linked
        // folder changes so peers hear about it in real time.
        let watcher_node = node.clone();
        tokio::spawn(async move {
            crate::network::watcher::spawn_watchers(watcher_node).await;
        });

        Ok(node)
    }

    /// Connect to a peer using their stored EndpointAddr JSON.
    /// Call this on startup for each known peer, and after pairing.
    /// Returns once the Hello/HelloAck handshake completes; the connection is
    /// then handed to the sync engine, which keeps it alive and pushes this
    /// device's shares to the peer.
    pub async fn connect_to_peer(
        self: &Arc<Self>,
        endpoint_addr_json: &str,
        display_name: &str,
    ) -> Result<()> {
        let addr: iroh::EndpointAddr = serde_json::from_str(endpoint_addr_json)
            .map_err(|e| anyhow::anyhow!("Invalid endpoint addr: {}", e))?;
        let conn = self.endpoint.connect(addr, SOFFIT_ALPN).await?;
        let mut send = conn.open_uni().await?;
        let msg = crate::network::protocol::SyncMessage::Hello {
            node_id: self.node_id.clone(),
            display_name: display_name.to_string(),
            endpoint_addr: serde_json::to_string(&self.endpoint.addr())?,
        };
        let bytes = serde_json::to_vec(&msg)?;
        send.write_all(&(bytes.len() as u32).to_be_bytes()).await?;
        send.write_all(&bytes).await?;
        send.finish()?;

        // A successful QUIC dial only proves that a transport connection was
        // opened.  Wait for the remote application to authenticate and store
        // our Hello before telling the UI that pairing succeeded.
        let mut recv = conn.accept_uni().await?;
        let mut len_buf = [0u8; 4];
        recv.read_exact(&mut len_buf).await?;
        let len = u32::from_be_bytes(len_buf) as usize;
        if len > 1024 * 1024 {
            anyhow::bail!("pairing acknowledgement is too large");
        }
        let mut buf = vec![0u8; len];
        recv.read_exact(&mut buf).await?;
        match serde_json::from_slice::<crate::network::protocol::SyncMessage>(&buf)? {
            crate::network::protocol::SyncMessage::HelloAck => {}
            _ => anyhow::bail!("remote device sent an invalid pairing acknowledgement"),
        }

        // Handshake complete — run the sync engine in the background so the
        // pairing command returns promptly while file transfers proceed.
        let remote_node_id = conn.remote_id().to_string();
        let shared = Arc::new(sync::SharedCtx::default());
        self.conns
            .lock()
            .expect("connection registry poisoned")
            .insert(remote_node_id.clone(), (conn.clone(), shared.clone()));
        let node = self.clone();
        let peer_name = display_name.to_string();
        tokio::spawn(async move {
            sync::run_peer_connection(conn, &node, &remote_node_id, &peer_name, shared).await;
        });
        Ok(())
    }

    /// Tell the gossip swarm that `removed_node_id` was unpaired, so the
    /// removed device and every other member forgets the pairing.
    pub async fn broadcast_peer_removed(&self, removed_node_id: &str) -> Result<()> {
        self.broadcast_gossip(&crate::network::protocol::SyncMessage::PeerRemoved {
            remover_node_id: self.node_id.clone(),
            removed_node_id: removed_node_id.to_string(),
        })
        .await
    }

    /// Serialize our EndpointAddr for use as a pairing ticket
    pub async fn node_addr_ticket(&self) -> Result<String> {
        // iroh 1.1: addr() is sync, returns EndpointAddr
        let addr: EndpointAddr = self.endpoint.addr();
        Ok(serde_json::to_string(&addr)?)
    }

    pub async fn broadcast_lock(&self, file_path: &str, peer_name: &str) -> Result<()> {
        self.broadcast_gossip(&crate::network::protocol::SyncMessage::LockAcquired {
            file_path: file_path.to_string(),
            peer_name: peer_name.to_string(),
        })
        .await
    }

    pub async fn broadcast_unlock(&self, file_path: &str) -> Result<()> {
        self.broadcast_gossip(&crate::network::protocol::SyncMessage::LockReleased {
            file_path: file_path.to_string(),
        })
        .await
    }

    async fn broadcast_gossip(&self, msg: &crate::network::protocol::SyncMessage) -> Result<()> {
        let topic = TopicId::from_bytes(crate::network::protocol::presence_topic());
        let bytes: bytes::Bytes = serde_json::to_vec(msg)?.into();
        // iroh-gossip 0.101: subscribe_and_join returns Result<GossipTopic>.
        // The topic swarm can only form when we know at least one peer that
        // also joined it, so bootstrap from every known peer's Endpoint ID.
        let peer_node_ids: Vec<String> = match rusqlite::Connection::open(&self.db_path) {
            Ok(conn) => match conn.prepare("SELECT node_id FROM peers") {
                Ok(mut stmt) => stmt
                    .query_map([], |row| row.get::<_, String>(0))
                    .map(|rows| rows.filter_map(|r| r.ok()).collect())
                    .unwrap_or_default(),
                Err(_) => Vec::new(),
            },
            Err(_) => Vec::new(),
        };
        let bootstrap: Vec<iroh::EndpointId> = peer_node_ids
            .iter()
            .filter_map(|id| iroh::EndpointId::from_str(id).ok())
            .collect();
        if let Ok(mut topic_handle) = self.gossip.subscribe_and_join(topic, bootstrap).await {
            topic_handle.broadcast(bytes).await.ok();
        }
        Ok(())
    }

    async fn accept_loop(self: Arc<Self>) {
        while let Some(incoming) = self.endpoint.accept().await {
            let node = self.clone();
            tokio::spawn(async move {
                let Ok(conn) = incoming.await else {
                    return;
                };
                // Route by negotiated ALPN. The gossip actor does not accept
                // connections itself, so inbound gossip connections must be
                // forwarded to it manually.
                if conn.alpn() == GOSSIP_ALPN {
                    let _ = node.gossip.handle_connection(conn).await;
                    return;
                }
                // SOFFIT_ALPN: authenticate the Hello, acknowledge it, then
                // hand the live connection to the sync engine.
                let Ok(mut recv) = conn.accept_uni().await else {
                    return;
                };
                let mut len_buf = [0u8; 4];
                let Ok(()) = recv.read_exact(&mut len_buf).await else {
                    return;
                };
                let len = u32::from_be_bytes(len_buf) as usize;
                if len == 0 || len > 1024 * 1024 {
                    return;
                }
                let mut buf = vec![0u8; len];
                let Ok(()) = recv.read_exact(&mut buf).await else {
                    return;
                };
                let authenticated_node_id = conn.remote_id().to_string();
                let Ok(crate::network::protocol::SyncMessage::Hello {
                    node_id,
                    display_name,
                    endpoint_addr,
                }) = serde_json::from_slice::<crate::network::protocol::SyncMessage>(&buf)
                else {
                    return;
                };
                // Do not trust an ID supplied in an application message.
                // Iroh has already authenticated the TLS endpoint identity.
                if node_id != authenticated_node_id {
                    return;
                }
                let _ = node.event_tx.send(NetworkEvent::PeerOnline {
                    node_id: node_id.clone(),
                    display_name: display_name.clone(),
                    endpoint_addr,
                });

                // Send the acknowledgement only after the event has been
                // emitted. This makes a successful pairing result meaningful
                // on both devices instead of optimistic.
                if let Ok(mut send) = conn.open_uni().await {
                    if let Ok(bytes) =
                        serde_json::to_vec(&crate::network::protocol::SyncMessage::HelloAck)
                    {
                        let _ = send.write_all(&(bytes.len() as u32).to_be_bytes()).await;
                        let _ = send.write_all(&bytes).await;
                        let _ = send.finish();
                    }
                }

                // The sync engine keeps this connection alive: it serves
                // inbound streams (files pushed to us) and pushes this
                // device's owned shares to the peer.
                let shared = Arc::new(sync::SharedCtx::default());
                node.conns
                    .lock()
                    .expect("connection registry poisoned")
                    .insert(
                        authenticated_node_id.clone(),
                        (conn.clone(), shared.clone()),
                    );
                sync::run_peer_connection(
                    conn,
                    &node,
                    &authenticated_node_id,
                    &display_name,
                    shared,
                )
                .await;
            });
        }
    }
}
