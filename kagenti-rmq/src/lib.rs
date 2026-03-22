//! kagenti-rmq: RabbitMQ event bus (replaces k8s informers)

use kagenti_api::types::AgentSummary;
use lapin::{options::*, types::FieldTable, BasicProperties, Channel, Connection, ConnectionProperties};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

const EXCHANGE: &str = "kagenti.events";
const QUEUE: &str = "kagenti.agent.events";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum AgentEvent {
    Created(AgentSummary),
    Updated(AgentSummary),
    Deleted { name: String, namespace: String },
}

pub struct EventBus {
    channel: Channel,
}

impl EventBus {
    pub async fn connect(uri: &str) -> Result<Self, lapin::Error> {
        let conn = Connection::connect(uri, ConnectionProperties::default()).await?;
        let ch = conn.create_channel().await?;
        ch.exchange_declare(EXCHANGE, lapin::ExchangeKind::Fanout,
            ExchangeDeclareOptions::default(), FieldTable::default()).await?;
        ch.queue_declare(QUEUE, QueueDeclareOptions::default(), FieldTable::default()).await?;
        ch.queue_bind(QUEUE, EXCHANGE, "", QueueBindOptions::default(), FieldTable::default()).await?;
        Ok(Self { channel: ch })
    }

    pub async fn publish(&self, event: &AgentEvent) -> Result<(), lapin::Error> {
        let payload = serde_json::to_vec(event).unwrap();
        self.channel.basic_publish(EXCHANGE, "", BasicPublishOptions::default(),
            &payload, BasicProperties::default().with_content_type("application/json".into()),
        ).await?.await?;
        Ok(())
    }

    pub async fn subscribe(&self) -> Result<mpsc::Receiver<AgentEvent>, lapin::Error> {
        let (tx, rx) = mpsc::channel(64);
        let mut consumer = self.channel.basic_consume(
            QUEUE, "kagenti-daemon", BasicConsumeOptions::default(), FieldTable::default(),
        ).await?;
        tokio::spawn(async move {
            use futures_lite::StreamExt;
            while let Some(Ok(delivery)) = consumer.next().await {
                if let Ok(event) = serde_json::from_slice::<AgentEvent>(&delivery.data) {
                    let _ = tx.send(event).await;
                }
                let _ = delivery.ack(BasicAckOptions::default()).await;
            }
        });
        Ok(rx)
    }
}
