//! C# `PluginManifestTests` (11), all ported. C# only sets/reads properties; the Rust tests also
//! round-trip through the `plugin.json` field names where that is the observable contract.

use std::collections::BTreeMap;

use arcana_plugin_api::manifest::{
    ManifestKeybindingDefinition, ManifestToolbarDefinition, ManifestToolbarItemDefinition,
};
use arcana_plugin_api::{
    ManifestCommandDefinition, ManifestConfigurationDefinition, ManifestConfigurationProperty,
    ManifestContributions, ManifestMenuDefinition, ManifestViewDefinition, PluginManifest,
    PluginType,
};
use serde_json::json;

// PluginManifest_Properties_ShouldBeSetCorrectly
#[test]
fn manifest_properties() {
    let m = PluginManifest {
        id: "arcana.module.order".into(),
        name: "Order Module".into(),
        version: "1.0.0".into(),
        description: Some("Manages orders".into()),
        author: Some("Arcana".into()),
        main: Some("OrderModule.dll".into()),
        plugin_class: Some("Arcana.Order.OrderPlugin".into()),
        plugin_type: "Module".into(),
        dependencies: Some(vec!["arcana.core".into()]),
        activation_events: Some(vec!["onStartup".into()]),
        icon: Some("icons/order.png".into()),
        l10n: Some(BTreeMap::from([("en".into(), "locales/en.json".into())])),
        ..PluginManifest::default()
    };
    let back = PluginManifest::from_json(&m.to_json().unwrap()).unwrap();
    assert_eq!(back, m);
    let v: serde_json::Value = serde_json::from_str(&m.to_json().unwrap()).unwrap();
    assert_eq!(v["pluginClass"], "Arcana.Order.OrderPlugin");
    assert_eq!(v["type"], "Module");
    assert_eq!(v["activationEvents"], json!(["onStartup"]));
    assert_eq!(back.activation_events(), ["onStartup"]);
    assert_eq!(back.parsed_type(), PluginType::Module);
    assert!(back.l10n.unwrap().contains_key("en"));
}

// PluginManifest_DefaultType_ShouldBeModule
#[test]
fn manifest_default_type() {
    assert_eq!(PluginManifest::default().plugin_type, "Module");
    let m = PluginManifest::from_json(r#"{"id":"test","name":"Test","version":"1.0.0"}"#).unwrap();
    assert_eq!(m.plugin_type, "Module");
}

// ManifestContributions_Properties_ShouldBeSetCorrectly
#[test]
fn contributions_properties() {
    let c = ManifestContributions {
        views: Some(vec![ManifestViewDefinition {
            id: "view1".into(),
            title_key: "title.key".into(),
            ..ManifestViewDefinition::default()
        }]),
        menus: Some(vec![ManifestMenuDefinition {
            id: "menu1".into(),
            location: "MainMenu".into(),
            ..ManifestMenuDefinition::default()
        }]),
        commands: Some(vec![ManifestCommandDefinition {
            id: "cmd1".into(),
            ..ManifestCommandDefinition::default()
        }]),
        toolbars: Some(vec![ManifestToolbarDefinition {
            id: "toolbar1".into(),
            ..ManifestToolbarDefinition::default()
        }]),
        keybindings: Some(vec![ManifestKeybindingDefinition {
            command: "cmd1".into(),
            key: "Ctrl+S".into(),
            ..ManifestKeybindingDefinition::default()
        }]),
        configuration: Some(ManifestConfigurationDefinition {
            title: Some("Settings".into()),
            ..ManifestConfigurationDefinition::default()
        }),
    };
    assert_eq!(c.views.as_ref().map(Vec::len), Some(1));
    assert_eq!(c.menus.as_ref().map(Vec::len), Some(1));
    assert_eq!(c.commands.as_ref().map(Vec::len), Some(1));
    assert_eq!(c.toolbars.as_ref().map(Vec::len), Some(1));
    assert_eq!(c.keybindings.as_ref().map(Vec::len), Some(1));
    assert!(c.configuration.is_some());
}

// ManifestViewDefinition_Properties_ShouldBeSetCorrectly
#[test]
fn view_definition_properties() {
    let v: ManifestViewDefinition = serde_json::from_value(json!({
        "id": "view.orders", "titleKey": "views.orders.title", "title": "Orders",
        "icon": "icons/list.png", "type": "Page",
        "viewClass": "Arcana.Order.Views.OrderListView",
        "viewModelClass": "Arcana.Order.ViewModels.OrderListViewModel",
        "category": "Operations", "categoryKey": "category.operations", "order": 10,
        "canHaveMultipleInstances": true, "moduleId": "arcana.module.order",
        "isModuleDefaultTab": true, "moduleTabOrder": 1
    }))
    .unwrap();
    assert_eq!(v.id, "view.orders");
    assert_eq!(v.title_key, "views.orders.title");
    assert_eq!(v.title.as_deref(), Some("Orders"));
    assert_eq!(v.view_type, "Page");
    assert_eq!(v.order, 10);
    assert!(v.can_have_multiple_instances);
    assert!(v.is_module_default_tab);
    assert_eq!(v.module_tab_order, 1);
}

// ManifestViewDefinition_DefaultType_ShouldBePage
#[test]
fn view_definition_default_type() {
    assert_eq!(ManifestViewDefinition::default().view_type, "Page");
    let v: ManifestViewDefinition =
        serde_json::from_value(json!({"id": "v", "titleKey": "k"})).unwrap();
    assert_eq!(v.view_type, "Page");
}

// ManifestMenuDefinition_Properties_ShouldBeSetCorrectly
#[test]
fn menu_definition_properties() {
    let m: ManifestMenuDefinition = serde_json::from_value(json!({
        "id": "menu.file.open", "location": "FileMenu", "titleKey": "menu.open",
        "title": "Open", "parentId": "menu.file", "icon": "icons/open.png",
        "shortcut": "Ctrl+O", "command": "file.open", "order": 5, "group": "file",
        "when": "workspaceOpened", "isSeparator": false, "moduleId": "arcana.module.core",
        "children": []
    }))
    .unwrap();
    assert_eq!(m.id, "menu.file.open");
    assert_eq!(m.location, "FileMenu");
    assert_eq!(m.command.as_deref(), Some("file.open"));
    assert_eq!(m.order, 5);
    assert!(!m.is_separator);
    assert_eq!(m.children.as_ref().map(Vec::len), Some(0));
}

// ManifestCommandDefinition_Properties_ShouldBeSetCorrectly
#[test]
fn command_definition_properties() {
    let c: ManifestCommandDefinition = serde_json::from_value(json!({
        "id": "file.open", "titleKey": "cmd.open", "title": "Open File",
        "category": "File", "icon": "icons/open.png", "enablement": "workspaceOpened"
    }))
    .unwrap();
    assert_eq!(c.id, "file.open");
    assert_eq!(c.title.as_deref(), Some("Open File"));
    assert_eq!(c.category.as_deref(), Some("File"));
    assert_eq!(c.enablement.as_deref(), Some("workspaceOpened"));
}

// ManifestToolbarDefinition_Properties_ShouldBeSetCorrectly
#[test]
fn toolbar_definition_properties() {
    let t = ManifestToolbarDefinition {
        id: "toolbar.main".into(),
        title_key: Some("toolbar.main.title".into()),
        title: Some("Main Toolbar".into()),
        items: Some(vec![ManifestToolbarItemDefinition {
            command: "file.save".into(),
            icon: Some("icons/save.png".into()),
            when: Some("documentOpen".into()),
            group: Some("file".into()),
        }]),
    };
    let items = t.items.unwrap();
    assert_eq!(t.id, "toolbar.main");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].command, "file.save");
    assert_eq!(items[0].group.as_deref(), Some("file"));
}

// ManifestKeybindingDefinition_Properties_ShouldBeSetCorrectly
#[test]
fn keybinding_definition_properties() {
    let k: ManifestKeybindingDefinition = serde_json::from_value(json!({
        "command": "file.save", "key": "Ctrl+S", "when": "documentOpen",
        "mac": "Cmd+S", "win": "Ctrl+S", "linux": "Ctrl+S"
    }))
    .unwrap();
    assert_eq!(k.command, "file.save");
    assert_eq!(k.key, "Ctrl+S");
    assert_eq!(k.mac.as_deref(), Some("Cmd+S"));
    assert_eq!(k.win.as_deref(), Some("Ctrl+S"));
    assert_eq!(k.linux.as_deref(), Some("Ctrl+S"));
}

// ManifestConfigurationDefinition_Properties_ShouldBeSetCorrectly
#[test]
fn configuration_definition_properties() {
    let c: ManifestConfigurationDefinition = serde_json::from_value(json!({
        "title": "Plugin Settings", "titleKey": "settings.title",
        "properties": {
            "maxItems": {"type": "number", "default": 100, "description": "Max items",
                         "descriptionKey": "settings.maxItems.desc", "minimum": 1, "maximum": 1000},
            "theme": {"type": "string", "default": "light", "enum": [{}],
                      "enumDescriptions": ["Light theme"]}
        }
    }))
    .unwrap();
    let props = c.properties.clone().unwrap();
    assert_eq!(c.title.as_deref(), Some("Plugin Settings"));
    assert_eq!(props.len(), 2);
    let max: &ManifestConfigurationProperty = &props["maxItems"];
    assert_eq!(max.prop_type, "number");
    assert_eq!(max.minimum, Some(1.0));
    assert_eq!(max.maximum, Some(1000.0));
    assert_eq!(
        props["theme"].enum_descriptions.as_ref().map(Vec::len),
        Some(1)
    );
}

// PluginManifest_WithContributes_ShouldSetCorrectly
#[test]
fn manifest_with_contributes() {
    let m = PluginManifest::from_json(
        r#"{"id":"test","name":"Test","version":"1.0.0","contributes":{"views":[{"id":"v1","titleKey":"k1"}]}}"#,
    )
    .unwrap();
    assert_eq!(
        m.contributes.and_then(|c| c.views).map(|v| v.len()),
        Some(1)
    );
}
