//! C# `PluginLocalizationIntegrationTests` (5) + `ContributionValidationIntegrationTests` (4).
//! C# uses its own `TestLocalizationService`; here that is the shared `FakeL10n`.

#[path = "support/host.rs"]
mod host;

use std::any::Any;
use std::path::Path;

use arcana_core::{AppError, AppResult};
use arcana_plugin_api::{
    CommandService, LocalizationService, MenuItemDefinition, MenuLocation, MenuRegistry, Plugin,
    PluginMetadata, PluginType, ViewDefinition, ViewRegistry, validate_menu_item, validate_view,
};
use arcana_plugin_host::plugin_base::{BasePlugin, PluginBase, PluginBehavior};
use arcana_plugin_host::services::command::CommandServiceImpl;
use arcana_plugin_host::services::menu::MenuRegistryImpl;
use arcana_plugin_host::services::view::ViewRegistryImpl;
use host::{Fixture, fixture, tmp, write};
use std::sync::Arc;

fn meta(id: &str) -> PluginMetadata {
    PluginMetadata::new(id, id, semver::Version::new(1, 0, 0), PluginType::Module)
}

struct Integration(String);
impl PluginBehavior for Integration {
    fn metadata(&self) -> PluginMetadata {
        meta(&self.0)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

struct Contrib {
    valid: bool,
}
impl PluginBehavior for Contrib {
    fn metadata(&self) -> PluginMetadata {
        meta(if self.valid {
            "valid.plugin"
        } else {
            "invalid.plugin"
        })
    }
    fn register_contributions(&mut self, base: &mut PluginBase) -> AppResult<()> {
        if !self.valid {
            return base.register_menu_item(MenuItemDefinition::new(
                "123.invalid.menu",
                "Invalid Menu",
                MenuLocation::MainMenu,
            ));
        }
        base.register_menu_item(MenuItemDefinition::new(
            "valid.menu",
            "Valid Menu",
            MenuLocation::MainMenu,
        ))?;
        base.register_view(ViewDefinition {
            title_key: Some("valid.page.title".into()),
            view_class_name: Some("object".into()),
            ..ViewDefinition::new("ValidPage", "Valid Page")
        })?;
        base.register_command("valid.command", |_| Ok(()))
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn activate<B: PluginBehavior>(f: &Fixture, b: B, path: &Path) -> (BasePlugin<B>, AppResult<()>) {
    let mut p = BasePlugin::new(b);
    let id = p.metadata().id.clone();
    let r = p.activate(&f.context(&id, path));
    (p, r)
}

// Plugin_LoadExternalLocalization_ShouldIntegrateWithLocalizationService
#[test]
fn load_external_localization_should_integrate_with_localization_service() {
    let f = fixture();
    let d = tmp();
    let loc = d.path().join("locales");
    write(
        &loc.join("en-US.json"),
        r#"{"greeting":"Hello","farewell":"Goodbye"}"#,
    );
    write(
        &loc.join("zh-TW.json"),
        r#"{"greeting":"你好","farewell":"再見"}"#,
    );
    let (p, r) = activate(&f, Integration("integration.test.plugin".into()), &loc);
    assert!(r.is_ok());
    assert_eq!(p.base().load_external_localization(Some(&loc)), 2);
    assert_eq!(
        f.l10n.get_for_plugin("integration.test.plugin", "greeting"),
        "Hello"
    );
}

// Plugin_RegisterInvalidContributions_ShouldThrowValidationException
#[test]
fn register_invalid_contributions_should_fail_validation() {
    let f = fixture();
    let d = tmp();
    let (p, r) = activate(&f, Contrib { valid: false }, d.path());
    assert!(matches!(r, Err(AppError::Validation { .. })), "{r:?}");
    assert_eq!(p.state(), arcana_plugin_api::PluginState::Error);
}

// Plugin_RegisterValidContributions_ShouldSucceed
#[test]
fn register_valid_contributions_should_succeed() {
    let f = fixture();
    let d = tmp();
    let (_p, r) = activate(&f, Contrib { valid: true }, d.path());
    assert!(r.is_ok());
    assert_eq!(f.menus.all_menu_items().len(), 1);
    assert_eq!(f.views.all_views().len(), 1);
    assert!(f.commands.has_command("valid.command"));
}

// MultiplePlugins_ShouldHaveSeparateLocalizations
#[test]
fn multiple_plugins_should_have_separate_localizations() {
    let f = fixture();
    let d = tmp();
    for n in ["plugin1", "plugin2"] {
        let dir = d.path().join(n);
        write(
            &dir.join("locales/en-US.json"),
            &format!(r#"{{"key":"{} Value"}}"#, n.replace("plugin", "Plugin ")),
        );
        let (p, r) = activate(&f, Integration(n.into()), &dir);
        assert!(r.is_ok());
        assert_eq!(p.base().load_external_localization(None), 1);
    }
    assert_eq!(f.l10n.get_for_plugin("plugin1", "key"), "Plugin 1 Value");
    assert_eq!(f.l10n.get_for_plugin("plugin2", "key"), "Plugin 2 Value");
}

// Plugin_Deactivate_ShouldCleanupSubscriptions
#[test]
fn deactivate_should_cleanup_subscriptions() {
    let f = fixture();
    let d = tmp();
    let (mut p, r) = activate(&f, Contrib { valid: true }, d.path());
    assert!(r.is_ok());
    assert_eq!(f.menus.all_menu_items().len(), 1);
    assert!(p.deactivate().is_ok());
    assert_eq!(f.menus.all_menu_items().len(), 0);
    assert_eq!(f.views.all_views().len(), 0);
    assert!(!f.commands.has_command("valid.command"));
}

// MenuRegistry_WithLogger_ShouldLogValidationWarnings — the warning comes from the validator
// the registry runs; registration still succeeds.
#[test]
fn menu_registry_registers_item_with_validation_warnings() {
    let reg = MenuRegistryImpl::new();
    let item = MenuItemDefinition::new("test.menu", "Test", MenuLocation::MainMenu).with_order(-5);
    assert_ne!(validate_menu_item(&item).warnings.len(), 0);
    assert!(reg.register_menu_item(item).is_ok());
    assert_eq!(reg.all_menu_items().len(), 1);
}

// ViewRegistry_WithLogger_ShouldLogValidationWarnings
#[test]
fn view_registry_registers_view_with_validation_warnings() {
    let reg = ViewRegistryImpl::new();
    let view = ViewDefinition {
        view_class_name: Some("object".into()),
        ..ViewDefinition::new("TestPage", "Test")
    };
    assert_ne!(validate_view(&view).warnings.len(), 0);
    assert!(reg.register_view(view).is_ok());
    assert_eq!(reg.all_views().len(), 1);
}

// AllRegistries_ValidationErrors_ShouldIncludeDetailedMessages
#[test]
fn all_registries_validation_errors_should_include_detailed_messages() {
    let errors = |r: AppResult<arcana_plugin_api::Subscription>| match r {
        Err(AppError::Validation { errors, .. }) => errors.len(),
        other => panic!("expected validation error, got {other:?}"),
    };
    assert!(
        errors(
            MenuRegistryImpl::new().register_menu_item(MenuItemDefinition::new(
                "123invalid",
                "",
                MenuLocation::MainMenu
            ))
        ) > 0
    );
    assert!(errors(ViewRegistryImpl::new().register_view(ViewDefinition::new("_invalid", ""))) > 0);
    assert!(
        errors(
            CommandServiceImpl::new()
                .register_command("", Arc::new(|_: &arcana_plugin_api::CommandArgs| Ok(())))
        ) > 0
    );
}

// MenuItemWithCommand_CommandNotRegistered_ShouldStillRegister
#[test]
fn menu_item_with_unregistered_command_should_still_register() {
    let menus = MenuRegistryImpl::new();
    let commands = CommandServiceImpl::new();
    assert!(
        menus
            .register_menu_item(
                MenuItemDefinition::new("test.menu", "Test", MenuLocation::MainMenu)
                    .with_command("future.command")
            )
            .is_ok()
    );
    assert!(
        commands
            .register_command(
                "future.command",
                Arc::new(|_: &arcana_plugin_api::CommandArgs| Ok(()))
            )
            .is_ok()
    );
    assert_eq!(menus.all_menu_items().len(), 1);
    assert!(commands.has_command("future.command"));
}
