//! `IMessageBus`, `IEventAggregator`, `ISharedStateStore`.
//!
//! The object-safe traits work on `dyn Any` keyed by `TypeId`; the `*Ext` traits give plugins
//! the typed C# shape (`Publish<T>`, `Subscribe<T>`, `Get<T>` ...).

use std::any::{Any, TypeId};
use std::sync::Arc;
use std::time::Duration;

use arcana_core::{AppError, AppResult, ErrorCode};
use chrono::{DateTime, Utc};

use crate::services::Subscription;

pub type AnyHandler = Arc<dyn Fn(&(dyn Any + Send + Sync)) + Send + Sync>;
pub type AnyRequestHandler =
    Arc<dyn Fn(&(dyn Any + Send + Sync)) -> AppResult<Box<dyn Any + Send>> + Send + Sync>;
pub type StateHandler = Arc<dyn Fn(Option<&(dyn Any + Send + Sync)>) + Send + Sync>;

/// C# `IMessageBus`.
pub trait MessageBus: Send + Sync {
    fn publish_any(&self, type_id: TypeId, message: &(dyn Any + Send + Sync));
    fn subscribe_any(&self, type_id: TypeId, handler: AnyHandler) -> Subscription;
    /// `Ok(None)` when no handler answers within `timeout` (C# returns `default`).
    ///
    /// # Errors
    /// The handler's error.
    fn request_any(
        &self,
        request_type: TypeId,
        response_type: TypeId,
        request: &(dyn Any + Send + Sync),
        timeout: Option<Duration>,
    ) -> AppResult<Option<Box<dyn Any + Send>>>;
    /// # Errors
    /// A handler is already registered for this request/response pair.
    fn register_handler_any(
        &self,
        request_type: TypeId,
        response_type: TypeId,
        handler: AnyRequestHandler,
    ) -> AppResult<Subscription>;
}

/// Typed helpers over [`MessageBus`].
pub trait MessageBusExt {
    fn publish<T: Any + Send + Sync>(&self, message: &T);
    fn subscribe<T: Any + Send + Sync>(
        &self,
        handler: impl Fn(&T) + Send + Sync + 'static,
    ) -> Subscription;
    /// # Errors
    /// The handler's error.
    fn request<Req: Any + Send + Sync, Resp: Any + Send>(
        &self,
        request: &Req,
        timeout: Option<Duration>,
    ) -> AppResult<Option<Resp>>;
    /// # Errors
    /// Duplicate handler.
    fn register_handler<Req: Any + Send + Sync, Resp: Any + Send>(
        &self,
        handler: impl Fn(&Req) -> AppResult<Resp> + Send + Sync + 'static,
    ) -> AppResult<Subscription>;
}

impl<B: MessageBus + ?Sized> MessageBusExt for B {
    fn publish<T: Any + Send + Sync>(&self, message: &T) {
        self.publish_any(TypeId::of::<T>(), message);
    }
    fn subscribe<T: Any + Send + Sync>(
        &self,
        handler: impl Fn(&T) + Send + Sync + 'static,
    ) -> Subscription {
        self.subscribe_any(
            TypeId::of::<T>(),
            Arc::new(move |m| {
                if let Some(t) = m.downcast_ref::<T>() {
                    handler(t);
                }
            }),
        )
    }
    fn request<Req: Any + Send + Sync, Resp: Any + Send>(
        &self,
        request: &Req,
        timeout: Option<Duration>,
    ) -> AppResult<Option<Resp>> {
        let out = self.request_any(TypeId::of::<Req>(), TypeId::of::<Resp>(), request, timeout)?;
        Ok(out.and_then(|b| b.downcast::<Resp>().ok()).map(|b| *b))
    }
    fn register_handler<Req: Any + Send + Sync, Resp: Any + Send>(
        &self,
        handler: impl Fn(&Req) -> AppResult<Resp> + Send + Sync + 'static,
    ) -> AppResult<Subscription> {
        self.register_handler_any(
            TypeId::of::<Req>(),
            TypeId::of::<Resp>(),
            Arc::new(move |m| {
                let req = m.downcast_ref::<Req>().ok_or_else(|| {
                    AppError::data(ErrorCode::InvalidFormat, "request type mismatch")
                })?;
                Ok(Box::new(handler(req)?) as Box<dyn Any + Send>)
            }),
        )
    }
}

/// C# `IApplicationEvent`.
pub trait ApplicationEvent: Any + Send + Sync {
    fn timestamp(&self) -> DateTime<Utc>;
    fn source_plugin_id(&self) -> Option<&str>;
}

/// C# `IEventAggregator`.
pub trait EventAggregator: Send + Sync {
    fn publish_any(&self, type_id: TypeId, event: &(dyn Any + Send + Sync));
    fn subscribe_any(&self, type_id: TypeId, handler: AnyHandler) -> Subscription;
}

/// Typed helpers over [`EventAggregator`].
pub trait EventAggregatorExt {
    fn publish<E: ApplicationEvent>(&self, event: &E);
    fn subscribe<E: ApplicationEvent>(
        &self,
        handler: impl Fn(&E) + Send + Sync + 'static,
    ) -> Subscription;
}

impl<A: EventAggregator + ?Sized> EventAggregatorExt for A {
    fn publish<E: ApplicationEvent>(&self, event: &E) {
        self.publish_any(TypeId::of::<E>(), event);
    }
    fn subscribe<E: ApplicationEvent>(
        &self,
        handler: impl Fn(&E) + Send + Sync + 'static,
    ) -> Subscription {
        self.subscribe_any(
            TypeId::of::<E>(),
            Arc::new(move |m| {
                if let Some(e) = m.downcast_ref::<E>() {
                    handler(e);
                }
            }),
        )
    }
}

/// C# `ISharedStateStore`.
pub trait SharedStateStore: Send + Sync {
    fn get_any(&self, key: &str) -> Option<Arc<dyn Any + Send + Sync>>;
    fn set_any(&self, key: &str, value: Arc<dyn Any + Send + Sync>);
    fn remove(&self, key: &str) -> bool;
    fn contains_key(&self, key: &str) -> bool;
    fn on_change_any(&self, key: &str, handler: StateHandler) -> Subscription;
}

/// Typed helpers over [`SharedStateStore`].
pub trait SharedStateStoreExt {
    /// C# `Get<T>`: `None` when missing or of another type.
    fn get<T: Any + Send + Sync + Clone>(&self, key: &str) -> Option<T>;
    fn set<T: Any + Send + Sync>(&self, key: &str, value: T);
    fn on_change<T: Any + Send + Sync + Clone>(
        &self,
        key: &str,
        handler: impl Fn(Option<T>) + Send + Sync + 'static,
    ) -> Subscription;
}

impl<S: SharedStateStore + ?Sized> SharedStateStoreExt for S {
    fn get<T: Any + Send + Sync + Clone>(&self, key: &str) -> Option<T> {
        self.get_any(key)
            .and_then(|v| v.downcast_ref::<T>().cloned())
    }
    fn set<T: Any + Send + Sync>(&self, key: &str, value: T) {
        self.set_any(key, Arc::new(value));
    }
    fn on_change<T: Any + Send + Sync + Clone>(
        &self,
        key: &str,
        handler: impl Fn(Option<T>) + Send + Sync + 'static,
    ) -> Subscription {
        self.on_change_any(
            key,
            Arc::new(move |v| handler(v.and_then(|v| v.downcast_ref::<T>().cloned()))),
        )
    }
}
