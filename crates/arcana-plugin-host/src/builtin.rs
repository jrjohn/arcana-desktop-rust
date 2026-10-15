//! Compile-time registration of built-in plugins (`inventory`), replacing C#
//! `App.xaml.cs` calling `RegisterBuiltInPlugin(new XxxPlugin())`.
//!
//! ```ignore
//! arcana_plugin_host::submit_builtin!(|| BasePlugin::boxed(MyPlugin::default()), 10);
//! ```

use arcana_plugin_api::Plugin;

/// One built-in plugin factory. `order` keeps registration deterministic (inventory itself
/// has no defined order).
pub struct BuiltinPlugin {
    pub factory: fn() -> Box<dyn Plugin>,
    pub order: i32,
}

inventory::collect!(BuiltinPlugin);

/// Every registered built-in, by `order` then id.
#[must_use]
pub fn builtin_plugins() -> Vec<Box<dyn Plugin>> {
    let mut list: Vec<(i32, Box<dyn Plugin>)> = inventory::iter::<BuiltinPlugin>()
        .map(|b| (b.order, (b.factory)()))
        .collect();
    list.sort_by(|a, b| (a.0, &a.1.metadata().id).cmp(&(b.0, &b.1.metadata().id)));
    list.into_iter().map(|(_, p)| p).collect()
}

/// Register a built-in plugin factory at compile time.
#[macro_export]
macro_rules! submit_builtin {
    ($factory:expr, $order:expr) => {
        $crate::inventory::submit! {
            $crate::builtin::BuiltinPlugin { factory: $factory, order: $order }
        }
    };
}
