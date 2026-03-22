//! Quick smoke test: publish an AgentEvent, receive it back.
use kagenti_api::types::{AgentSummary, AgentStatus, ResourceLabels};
use kagenti_rmq::{AgentEvent, EventBus};

#[tokio::main]
async fn main() {
    let bus = EventBus::connect("amqp://kagenti:kagenti@localhost:5672/%2Fmonster").await.expect("connect");
    let mut rx = bus.subscribe().await.expect("subscribe");

    let event = AgentEvent::Created(AgentSummary {
        name: "test-rmq".into(), namespace: "default".into(),
        description: "RMQ test".into(), status: AgentStatus::Running,
        labels: ResourceLabels { protocol: None, framework: None, kind: None },
        workload_type: None, created_at: None,
    });

    bus.publish(&event).await.expect("publish");
    println!("published: {event:?}");

    match tokio::time::timeout(std::time::Duration::from_secs(3), rx.recv()).await {
        Ok(Some(got)) => println!("received: {got:?}\n✅ RMQ round-trip works"),
        Ok(None) => println!("channel closed"),
        Err(_) => println!("timeout — no message received"),
    }
}
