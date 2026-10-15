//! C# `FlowChartPlugin` + `FlowChartNavGraph`: registers the editor view, Tools menu,
//! function-tree / quick-access items and the three commands.
//!
//! Depends only on `arcana-plugin-api`; the host side registers it as a built-in.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use arcana_core::AppResult;
use arcana_plugin_api::{
    MenuItemDefinition, MenuLocation, Plugin, PluginContext, PluginMetadata, PluginState,
    PluginType, ViewDefinition, ViewType,
};

pub const PLUGIN_ID: &str = "arcana.plugin.flowchart";
/// C# `FlowChartNavGraph.Routes.Editor`.
pub const EDITOR_ROUTE: &str = "FlowChartEditorPage";
/// The C# `plugin.json`, kept for the manifest-driven parts (activation events, contributes).
pub const MANIFEST_JSON: &str = include_str!("../plugin.json");

const LOCALES: [(&str, &str); 3] = [
    ("en-US", include_str!("../locales/en-US.json")),
    ("zh-TW", include_str!("../locales/zh-TW.json")),
    ("ja-JP", include_str!("../locales/ja-JP.json")),
];

/// C# `EditorAction` / `EditorArgs` — the navigation parameter of [`EDITOR_ROUTE`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorArgs {
    New,
    Open(String),
    OpenDialog,
    Sample,
}

/// C# `FlowChartPlugin`.
#[derive(Debug)]
pub struct FlowChartPlugin {
    metadata: PluginMetadata,
    state: PluginState,
    context: Option<PluginContext>,
}

impl Default for FlowChartPlugin {
    fn default() -> Self {
        let mut metadata = PluginMetadata::new(
            PLUGIN_ID,
            "FlowChart",
            semver::Version::new(1, 0, 0),
            PluginType::Module,
        );
        metadata.description =
            Some("Flowchart drawing and editing plugin with Draw.io compatibility".to_string());
        metadata.author = Some("Arcana Team".to_string());
        Self {
            metadata,
            state: PluginState::NotLoaded,
            context: None,
        }
    }
}

fn menu(
    id: &str,
    title: &str,
    location: MenuLocation,
    parent: Option<&str>,
    icon: &str,
    order: i32,
    command: Option<&str>,
) -> MenuItemDefinition {
    let mut m = MenuItemDefinition::new(id, title, location).with_order(order);
    m.parent_id = parent.map(str::to_string);
    m.icon = Some(icon.to_string());
    m.command = command.map(str::to_string);
    m
}

impl FlowChartPlugin {
    fn l(ctx: &PluginContext, key: &str) -> String {
        ctx.localization.get_for_plugin(PLUGIN_ID, key)
    }

    fn register(ctx: &PluginContext) -> AppResult<()> {
        ctx.track(ctx.views.register_view(ViewDefinition {
            id: EDITOR_ROUTE.to_string(),
            title: Self::l(ctx, "flowchart.editor"),
            title_key: Some("flowchart.editor".to_string()),
            icon: Some("\u{E8FD}".to_string()),
            view_type: ViewType::Page,
            view_class_name: Some("FlowChartEditorPage".to_string()),
            category: Some(Self::l(ctx, "menu.tools")),
            can_have_multiple_instances: true,
            ..ViewDefinition::default()
        })?);
        let main = MenuLocation::MainMenu;
        let mut quick = menu(
            "quick.newFlowChart",
            &Self::l(ctx, "flowchart.new"),
            MenuLocation::QuickAccess,
            None,
            "\u{E8FD}",
            10,
            Some("flowchart.new"),
        );
        quick.group = Some("tools".to_string());
        ctx.track(ctx.menus.register_menu_items(vec![
            menu(
                "menu.tools.flowchart",
                &Self::l(ctx, "flowchart.title"),
                main,
                Some("menu.tools"),
                "\u{E8FD}",
                10,
                None,
            ),
            menu(
                "menu.tools.flowchart.new",
                &Self::l(ctx, "flowchart.new"),
                main,
                Some("menu.tools.flowchart"),
                "\u{E710}",
                1,
                Some("flowchart.new"),
            ),
            menu(
                "menu.tools.flowchart.open",
                &Self::l(ctx, "flowchart.open"),
                main,
                Some("menu.tools.flowchart"),
                "\u{E8E5}",
                2,
                Some("flowchart.open"),
            ),
            menu(
                "menu.tools.flowchart.sample",
                &Self::l(ctx, "flowchart.sample"),
                main,
                Some("menu.tools.flowchart"),
                "\u{E8F1}",
                3,
                Some("flowchart.sample"),
            ),
            menu(
                "tree.flowchart",
                &Self::l(ctx, "flowchart.title"),
                MenuLocation::FunctionTree,
                None,
                "\u{E8FD}",
                50,
                Some("flowchart.new"),
            ),
            quick,
        ])?);
        for (cmd, args) in [
            ("flowchart.new", EditorArgs::New),
            ("flowchart.open", EditorArgs::OpenDialog),
            ("flowchart.sample", EditorArgs::Sample),
        ] {
            let nav = ctx.nav_graph.clone();
            let sub = ctx.commands.register_command(
                cmd,
                Arc::new(move |_| {
                    nav.to_new_tab(EDITOR_ROUTE, Some(Box::new(args.clone())));
                    Ok(())
                }),
            )?;
            ctx.track(sub);
        }
        Ok(())
    }
}

impl Plugin for FlowChartPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    fn state(&self) -> PluginState {
        self.state
    }

    fn activate(&mut self, context: &PluginContext) -> AppResult<()> {
        self.state = PluginState::Activating;
        self.context = Some(context.clone());
        match Self::register(context) {
            Ok(()) => {
                self.state = PluginState::Active;
                Ok(())
            }
            Err(e) => {
                self.state = PluginState::Error;
                Err(e)
            }
        }
    }

    fn deactivate(&mut self) -> AppResult<()> {
        self.state = PluginState::Deactivating;
        if let Some(c) = &self.context {
            c.dispose_subscriptions();
        }
        self.state = PluginState::Deactivated;
        Ok(())
    }

    fn resources(&self) -> BTreeMap<String, BTreeMap<String, String>> {
        LOCALES
            .iter()
            .filter_map(|(c, json)| {
                serde_json::from_str::<HashMap<String, String>>(json)
                    .ok()
                    .map(|m| ((*c).to_string(), m.into_iter().collect()))
            })
            .collect()
    }
}
