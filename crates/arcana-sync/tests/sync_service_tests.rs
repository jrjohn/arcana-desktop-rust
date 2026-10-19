//! New (C# has no `SyncService` implementation): the queue-backed service over the loopback
//! transport — two replicas converge, failures are retried, state and events are reported.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use arcana_core::{AppError, AppResult, ErrorCode, Repository, Timestamp, utc_now};
use arcana_data::{Db, SqliteRepository, SyncQueueItem, SyncStatus};
use arcana_sync::{
    ChangeApplier, ConflictResolver, LoopbackHub, Mergeable, QueueSyncService, SyncChange,
    SyncCompleted, SyncMeta, SyncOperationType, SyncOptions, SyncService, SyncServiceExt,
    SyncState, SyncStateChanged, SyncTransport, SyncableEntity, VectorClock,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Wire {
    id: String,
    title: String,
    clock: VectorClock,
    at: Timestamp,
}

impl Mergeable for Wire {
    fn entity_id(&self) -> String {
        self.id.clone()
    }
}

/// A replica's document store; applies remote versions through the conflict resolver.
#[derive(Default)]
struct Store {
    docs: Mutex<BTreeMap<String, Wire>>,
    fail: Mutex<bool>,
}

#[async_trait]
impl ChangeApplier for Store {
    async fn apply(&self, change: &SyncChange) -> AppResult<()> {
        if *self.fail.lock().unwrap() {
            return Err(AppError::data(ErrorCode::ConflictError, "apply failed"));
        }
        let remote: Wire = serde_json::from_str(change.payload.as_deref().unwrap_or("null"))
            .map_err(|e| AppError::data(ErrorCode::DataCorruption, e.to_string()))?;
        let mut docs = self.docs.lock().unwrap();
        let merged = match docs.get(&remote.id) {
            None => remote,
            Some(local) => {
                let r = ConflictResolver::new("any").resolve(
                    local.clone(),
                    remote.clone(),
                    (&local.clock, &remote.clock),
                    (local.at, remote.at),
                );
                Wire {
                    clock: local.clock.merge(&remote.clock),
                    ..r.result
                }
            }
        };
        docs.insert(merged.id.clone(), merged);
        Ok(())
    }
}

#[derive(Serialize)]
struct Item {
    id: String,
    #[serde(skip)]
    meta: SyncMeta,
}
impl SyncableEntity for Item {
    fn sync_id(&self) -> String {
        self.id.clone()
    }
    fn meta(&self) -> &SyncMeta {
        &self.meta
    }
    fn meta_mut(&mut self) -> &mut SyncMeta {
        &mut self.meta
    }
}

struct Replica {
    node: String,
    db: Db,
    store: Arc<Store>,
    svc: QueueSyncService,
}

impl Replica {
    async fn new(node: &str, hub: &LoopbackHub, options: Option<SyncOptions>) -> Self {
        let db = Db::in_memory().await.unwrap();
        let store = Arc::new(Store::default());
        let svc = QueueSyncService::new(
            db.clone(),
            Arc::new(hub.transport()),
            store.clone(),
            options.unwrap_or_else(|| SyncOptions::new(node)),
        );
        Self {
            node: node.into(),
            db,
            store,
            svc,
        }
    }

    async fn edit(&self, id: &str, title: &str) {
        let doc = {
            let mut docs = self.store.docs.lock().unwrap();
            let clock = docs
                .get(id)
                .map(|d| d.clock.clone())
                .unwrap_or_default()
                .increment(&self.node);
            let doc = Wire {
                id: id.into(),
                title: title.into(),
                clock,
                at: utc_now(),
            };
            docs.insert(id.into(), doc.clone());
            doc
        };
        self.svc
            .queue_for_sync(
                "Doc",
                id,
                Some(serde_json::to_string(&doc).unwrap()),
                SyncOperationType::Update,
            )
            .await
            .unwrap();
    }

    fn titles(&self) -> BTreeMap<String, String> {
        self.store
            .docs
            .lock()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.title.clone()))
            .collect()
    }

    async fn queue(&self) -> Vec<SyncQueueItem> {
        SqliteRepository::<SyncQueueItem>::new(self.db.clone())
            .get_all()
            .await
            .unwrap()
    }
}

#[tokio::test]
async fn two_replicas_converge_over_loopback() {
    let hub = LoopbackHub::new();
    let a = Replica::new("A", &hub, None).await;
    let b = Replica::new("B", &hub, None).await;
    a.edit("doc1", "A1").await;
    b.edit("doc2", "B1").await;
    // Concurrent edit of the same document; B writes later so LWW picks B.
    a.edit("doc3", "from A").await;
    tokio::time::sleep(Duration::from_millis(5)).await;
    b.edit("doc3", "from B").await;
    assert_eq!(a.svc.pending_count().await.unwrap(), 2);

    a.svc.sync_now().await.unwrap();
    b.svc.sync_now().await.unwrap();
    a.svc.sync_now().await.unwrap();

    assert_eq!(a.titles(), b.titles());
    assert_eq!(a.titles().get("doc3").map(String::as_str), Some("from B"));
    assert_eq!(a.titles().len(), 3);
    assert_eq!(a.svc.pending_count().await.unwrap(), 0);
    assert!(
        a.queue()
            .await
            .iter()
            .all(|i| i.status == SyncStatus::Completed)
    );
    assert_eq!(a.svc.state(), SyncState::Idle);
    assert!(a.svc.last_sync_time().is_some());
    assert_eq!(hub.log().unwrap().len(), 4);
}

#[tokio::test]
async fn offline_push_marks_rows_failed_and_retries_later() {
    let hub = LoopbackHub::new();
    let a = Replica::new("A", &hub, None).await;
    let states = Arc::new(Mutex::new(Vec::<SyncStateChanged>::new()));
    let done = Arc::new(Mutex::new(Vec::<SyncCompleted>::new()));
    let (s, d) = (states.clone(), done.clone());
    a.svc
        .on_state_changed(Box::new(move |e| s.lock().unwrap().push(e.clone())));
    a.svc
        .on_sync_completed(Box::new(move |e| d.lock().unwrap().push(e.clone())));

    a.edit("doc1", "x").await;
    hub.set_online(false).unwrap();
    let err = a.svc.sync_now().await.unwrap_err();
    assert_eq!(err.code(), ErrorCode::NetworkUnavailable);
    assert_eq!(a.svc.state(), SyncState::Offline);
    let row = &a.queue().await[0];
    assert_eq!((row.status, row.retry_count), (SyncStatus::Failed, 1));
    assert_eq!(row.last_error.as_deref(), Some("loopback hub is offline"));
    assert_eq!(a.svc.pending_count().await.unwrap(), 1);

    hub.set_online(true).unwrap();
    a.svc.sync_now().await.unwrap();
    assert_eq!(a.svc.state(), SyncState::Idle);
    let row = &a.queue().await[0];
    assert_eq!(
        (row.status, row.last_error.clone()),
        (SyncStatus::Completed, None)
    );

    let states: Vec<_> = states.lock().unwrap().iter().map(|e| e.new_state).collect();
    assert_eq!(
        states,
        [
            SyncState::Syncing,
            SyncState::Offline,
            SyncState::Syncing,
            SyncState::Idle
        ]
    );
    let done = done.lock().unwrap();
    assert_eq!(done.len(), 2);
    assert!(!done[0].success && done[0].items_failed == 1);
    assert!(done[0].error_message.is_some());
    assert!(done[1].success && done[1].items_synced == 1);
}

#[tokio::test]
async fn retry_limit_excludes_rows() {
    let hub = LoopbackHub::new();
    let mut opts = SyncOptions::new("A");
    opts.max_retries = 1;
    let a = Replica::new("A", &hub, Some(opts)).await;
    a.edit("doc1", "x").await;
    hub.set_online(false).unwrap();
    assert!(a.svc.sync_now().await.is_err());
    assert_eq!(a.svc.pending_count().await.unwrap(), 0);
    assert_eq!(a.svc.options().max_retries, 1);
}

#[tokio::test]
async fn pull_failure_and_apply_failure_report_errors() {
    let hub = LoopbackHub::new();
    let a = Replica::new("A", &hub, None).await;
    let b = Replica::new("B", &hub, None).await;
    a.edit("doc1", "x").await;
    a.svc.sync_now().await.unwrap();

    *b.store.fail.lock().unwrap() = true;
    let err = b.svc.sync_now().await.unwrap_err();
    assert_eq!(err.code(), ErrorCode::ConflictError);
    assert_eq!(b.svc.state(), SyncState::Error);

    // Offline during pull (nothing to push): Offline state.
    hub.set_online(false).unwrap();
    assert!(b.svc.sync_now().await.is_err());
    assert_eq!(b.svc.state(), SyncState::Offline);
}

#[tokio::test]
async fn start_runs_in_background_and_stop_halts() {
    let hub = LoopbackHub::new();
    let mut opts = SyncOptions::new("A");
    opts.interval = Duration::from_millis(10);
    let a = Replica::new("A", &hub, Some(opts)).await;
    a.edit("doc1", "x").await;
    assert!(!a.svc.is_running());
    a.svc.start().await.unwrap();
    a.svc.start().await.unwrap(); // idempotent
    assert!(a.svc.is_running());
    for _ in 0..100 {
        if a.svc.pending_count().await.unwrap() == 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(a.svc.pending_count().await.unwrap(), 0);
    a.svc.stop().await.unwrap();
    assert!(!a.svc.is_running());
    a.svc.stop().await.unwrap();
}

#[tokio::test]
async fn queue_validation_and_typed_queueing() {
    let hub = LoopbackHub::new();
    let a = Replica::new("A", &hub, None).await;
    let e = a
        .svc
        .queue_for_sync(" ", "1", None, SyncOperationType::Create)
        .await
        .unwrap_err();
    assert_eq!(e.code(), ErrorCode::ValidationFailed);

    let item = Item {
        id: "i-1".into(),
        meta: SyncMeta::default(),
    };
    a.svc
        .queue_entity("Item", &item, SyncOperationType::Delete)
        .await
        .unwrap();
    let rows = a.queue().await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].entity_id, "i-1");
    assert_eq!(rows[0].payload.as_deref(), Some(r#"{"id":"i-1"}"#));
    assert_eq!(rows[0].operation, arcana_data::SyncOperation::Delete);

    // Nothing to push and nothing to pull is a successful round.
    a.svc.sync_now().await.unwrap();
    a.svc.sync_now().await.unwrap();
    assert_eq!(a.svc.state(), SyncState::Idle);
}

#[tokio::test]
async fn options_defaults_and_hub_transport_directly() {
    let o = SyncOptions::new("n");
    assert_eq!(o.interval, Duration::from_mins(5));
    assert_eq!(o.max_retries, 5);
    let hub = LoopbackHub::new();
    let t = hub.transport();
    let ch = SyncChange {
        entity_type: "Doc".into(),
        entity_id: "1".into(),
        operation: SyncOperationType::Create,
        payload: None,
        origin_node: "x".into(),
    };
    t.push(std::slice::from_ref(&ch)).await.unwrap();
    assert_eq!(t.pull("y").await.unwrap(), std::slice::from_ref(&ch));
    assert_eq!(t.pull("y").await.unwrap().len(), 0);
    assert_eq!(t.pull("x").await.unwrap().len(), 0);
    assert_eq!(SyncState::default(), SyncState::Idle);
}
