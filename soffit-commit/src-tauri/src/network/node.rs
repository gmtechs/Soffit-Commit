use anyhow::Result;
use iroh::{Endpoint, EndpointAddr, SecretKey, endpoint::presets};
use iroh_gossip::net::Gossip;
use iroh_gossip::proto::TopicId;
use std::sync::Arc;
use tokio::sync::broadcast;
use serde::{Deserialize, Serialize};
use crate::network::protocol::SOFFIT_ALPN;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum NetworkEvent {
    PeerOnline   { node_id: String, display_name: String },
    PeerOffline  { node_id: String },
    LockAcquired { file_path: String, peer_name: String },
    LockReleased { file_path: String },
    PermissionChanged { share_id: String, peer_id: String, level: String },
    SyncComplete { share_id: String, path: String },
    SyncError    { path: String, error: String },
}

pub struct IrohNode {
    pub endpoint: Endpoint,
    pub gossip: Arc<Gossip>,
    pub node_id: String,
    pub event_tx: broadcast::Sender<NetworkEvent>,
}

impl IrohNode {
    pub async fn start(secret_key: SecretKey) -> Result<Arc<Self>> {
        let endpoint = Endpoint::builder(presets::N0)
            .secret_key(secret_key)
            .alpns(vec![SOFFIT_ALPN.to_vec()])
            .bind()
            .await?;

        let node_id = endpoint.id().to_string();
        let gossip = Arc::new(Gossip::builder().spawn(endpoint.clone()));
        let (event_tx, _) = broadcast::channel(256);

        let node = Arc::new(IrohNode { endpoint, gossip, node_id, event_tx });

        let node_clone = node.clone();
        tokio::spawn(async move { node_clone.accept_loop().await; });

        Ok(node)
    }

    /// Connect to a peer using their stored EndpointAddr JSON.
    /// Call this on startup for each known peer, and after pairing.
    pub async fn connect_to_peer(&self, endpoint_addr_json: &str, display_name: &str) -> Result<()> {
        let addr: iroh::EndpointAddr = serde_json::from_str(endpoint_addr_json)
            .map_err(|e| anyhow::anyhow!("Invalid endpoint addr: {}", e))?;
        let conn = self.endpoint.connect(addr, SOFFIT_ALPN).await?;
        let mut send = conn.open_uni().await?;
        let msg = crate::network::protocol::SyncMessage::Hello {
            node_id: self.node_id.clone(),
            display_name: display_name.to_string(),
        };
        let bytes = serde_json::to_vec(&msg)?;
        send.write_all(&(bytes.len() as u32).to_be_bytes()).await?;
        send.write_all(&bytes).await?;
        send.finish()?;
        Ok(())
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
        }).await
    }

    pub async fn broadcast_unlock(&self, file_path: &str) -> Result<()> {
        self.broadcast_gossip(&crate::network::protocol::SyncMessage::LockReleased {
            file_path: file_path.to_string(),
        }).await
    }

    async fn broadcast_gossip(&self, msg: &crate::network::protocol::SyncMessage) -> Result<()> {
        let topic = TopicId::from_bytes(crate::network::protocol::presence_topic());
        let bytes: bytes::Bytes = serde_json::to_vec(msg)?.into();
        // iroh-gossip 0.101: subscribe_and_join returns Result<GossipTopic>
        if let Ok(mut topic_handle) = self.gossip.subscribe_and_join(topic, vec![]).await {
            topic_handle.broadcast(bytes).await.ok();
        }
        Ok(())
    }

    async fn accept_loop(&self) {
        while let Some(incoming) = self.endpoint.accept().await {
            let event_tx = self.event_tx.clone();
            tokio::spawn(async move {
                if let Ok(conn) = incoming.await {
                    if let Ok(mut recv) = conn.accept_uni().await {
                        let mut len_buf = [0u8; 4];
                        if recv.read_exact(&mut len_buf).await.is_ok() {
                            let len = u32::from_be_bytes(len_buf) as usize;
                            let mut buf = vec![0u8; len.min(1024 * 1024)];
                            if recv.read_exact(&mut buf).await.is_ok() {
                                if let Ok(
                                    crate::network::protocol::SyncMessage::Hello { node_id, display_name }
                                ) = serde_json::from_slice::<crate::network::protocol::SyncMessage>(&buf) {
                                    event_tx.send(NetworkEvent::PeerOnline { node_id, display_name }).ok();
                                }
                            }
                        }
                    }
                }
            });
        }
    }
}
