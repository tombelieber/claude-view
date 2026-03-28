//! Classification scheduler: priority queue, dedup, concurrency, omlx_ready gate.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

use super::client::{ConversationTurn, OmlxClient};
use super::SessionPhase;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    New = 0,
    Transition = 1,
    Steady = 2,
}

pub struct ClassifyRequest {
    pub session_id: String,
    pub priority: Priority,
    pub turns: Vec<ConversationTurn>,
    pub temperature: f32,
    pub generation: u64,
}

pub struct ClassifyResult {
    pub session_id: String,
    pub phase: SessionPhase,
    pub scope: Option<String>,
}

/// Run the scheduler as a tokio task.
pub async fn run_scheduler(
    mut classify_rx: mpsc::Receiver<ClassifyRequest>,
    result_tx: mpsc::Sender<ClassifyResult>,
    client: Arc<OmlxClient>,
    omlx_ready: Arc<AtomicBool>,
    max_concurrent: usize,
) {
    let mut queue: Vec<ClassifyRequest> = Vec::new();
    let mut latest_gen: HashMap<String, u64> = HashMap::new();
    let (done_tx, mut done_rx) = mpsc::channel::<()>(max_concurrent);
    let mut in_flight: usize = 0;

    loop {
        tokio::select! {
            req = classify_rx.recv() => {
                let Some(req) = req else { break };
                latest_gen.insert(req.session_id.clone(), req.generation);
                let pos = queue.iter().position(|r| r.priority > req.priority).unwrap_or(queue.len());
                queue.insert(pos, req);
                drain_queue(&mut queue, &latest_gen, &client, &result_tx, &done_tx, &omlx_ready, &mut in_flight, max_concurrent).await;
            }
            _ = done_rx.recv() => {
                in_flight = in_flight.saturating_sub(1);
                drain_queue(&mut queue, &latest_gen, &client, &result_tx, &done_tx, &omlx_ready, &mut in_flight, max_concurrent).await;
            }
        }
    }
}

async fn drain_queue(
    queue: &mut Vec<ClassifyRequest>,
    latest_gen: &HashMap<String, u64>,
    client: &Arc<OmlxClient>,
    result_tx: &mpsc::Sender<ClassifyResult>,
    done_tx: &mpsc::Sender<()>,
    omlx_ready: &Arc<AtomicBool>,
    in_flight: &mut usize,
    max_concurrent: usize,
) {
    if !omlx_ready.load(Ordering::Relaxed) {
        return;
    }

    while *in_flight < max_concurrent {
        let req = loop {
            let Some(req) = queue.first() else { return };
            let is_stale = latest_gen
                .get(&req.session_id)
                .map(|&g| g > req.generation)
                .unwrap_or(false);
            if is_stale {
                queue.remove(0);
                continue;
            }
            break queue.remove(0);
        };

        *in_flight += 1;
        let client = client.clone();
        let result_tx = result_tx.clone();
        let done_tx = done_tx.clone();
        let session_id = req.session_id.clone();
        let temp = req.temperature;

        tokio::spawn(async move {
            if let Some((phase, scope)) = client.classify(&req.turns, temp).await {
                let _ = result_tx
                    .send(ClassifyResult {
                        session_id,
                        phase,
                        scope,
                    })
                    .await;
            }
            let _ = done_tx.send(()).await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_ordering() {
        assert!(Priority::New < Priority::Transition);
        assert!(Priority::Transition < Priority::Steady);
    }

    #[test]
    fn classify_request_fields() {
        use super::super::client::Role;
        let req = ClassifyRequest {
            session_id: "test".into(),
            priority: Priority::New,
            turns: vec![ConversationTurn {
                role: Role::User,
                text: "hello".into(),
                tools: vec![],
            }],
            temperature: 0.2,
            generation: 1,
        };
        assert_eq!(req.session_id, "test");
        assert_eq!(req.priority, Priority::New);
        assert_eq!(req.generation, 1);
    }
}
