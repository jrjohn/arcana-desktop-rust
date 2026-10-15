//! Service contracts a plugin reaches through its [`crate::PluginContext`].

mod bus;
mod command;
mod localization;
mod menu;
mod nav;
mod subscription;
mod view;
mod window;

pub use bus::{
    AnyHandler, AnyRequestHandler, ApplicationEvent, ApplicationEventBase, EventAggregator,
    EventAggregatorExt, MessageBus, MessageBusExt, SharedStateStore, SharedStateStoreExt,
    StateHandler,
};
pub use command::{CommandArgs, CommandDefinition, CommandHandler, CommandService};
pub use localization::{
    CultureChanged, CultureChangedHandler, LocalizationService, format_template,
};
pub use menu::{MenuItemDefinition, MenuLocation, MenuRegistry};
pub use nav::{NavGraph, NavigationType};
pub use subscription::Subscription;
pub use view::{ViewDefinition, ViewFactory, ViewInstance, ViewRegistry, ViewType};
pub use window::{MessageLevel, WindowRequest, WindowService};
