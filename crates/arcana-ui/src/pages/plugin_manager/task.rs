//! Plugin manager tasks: everything that touches the plugin host.

use std::path::Path;

use arcana_plugin_api::PluginOperationResult;
use arcana_plugin_api::schema::settings_form;

use super::{Input, Op, PluginRow, Task};
use crate::services::UiServices;

fn setting_key(plugin_id: &str, key: &str) -> String {
    format!("plugin.{plugin_id}.{key}")
}

fn rows(svc: &UiServices) -> Vec<PluginRow> {
    let host = &svc.plugins.host;
    host.all_plugins()
        .into_iter()
        .map(|p| {
            let health = host.health_status(&p.id);
            let settings = host
                .manifest(&p.id)
                .map(|m| settings_form(&m))
                .unwrap_or_default()
                .into_iter()
                .map(|f| {
                    let v = svc
                        .settings
                        .get_value(&setting_key(&p.id, &f.key))
                        .unwrap_or_else(|| f.default.clone());
                    (f, v)
                })
                .collect();
            let pending = host.permissions.pending(&p.id);
            PluginRow {
                version: p.version.to_string(),
                description: p.description.clone().unwrap_or_default(),
                kind: p.plugin_type.name().to_string(),
                author: p.author.clone(),
                state: p.state,
                health: health.state,
                error_count: health.error_count,
                built_in: p.is_built_in,
                pending_permissions: if pending.is_empty() {
                    String::new()
                } else {
                    pending.to_string()
                },
                granted_permissions: host.permissions.permissions(&p.id).to_string(),
                settings,
                name: p.name,
                id: p.id,
            }
        })
        .collect()
}

fn done(r: &PluginOperationResult) -> Input {
    Input::Done(r.success, r.message.clone().unwrap_or_default())
}

async fn op(op: Op, svc: &UiServices) -> Input {
    let host = &svc.plugins.host;
    match op {
        Op::Activate(id) => done(&host.activate_op(&id)),
        Op::Deactivate(id) => done(&host.deactivate_op(&id)),
        Op::Reload(id) => done(&host.reload(&id)),
        Op::Install(path) => done(&host.install(Path::new(&path)).await),
        Op::Upgrade(id, path) => done(&host.upgrade(&id, Path::new(&path)).await),
        Op::Rollback(id, v) => done(&host.rollback(&id, &v).await),
        Op::Uninstall(id) => done(&host.uninstall(&id).await),
        Op::Approve(id) => done(&host.approve_permissions(&id)),
        Op::CheckHealth(id) => {
            let all = host.check_all_health().await;
            let s = all.iter().find(|s| s.plugin_id == id).cloned();
            match s {
                Some(s) => Input::Done(true, format!("{}: {}", s.plugin_name, s.state.name())),
                None => Input::Done(false, format!("Plugin not found: {id}")),
            }
        }
        Op::CheckAllHealth => {
            let all = host.check_all_health().await;
            Input::Done(true, format!("Checked {} plugins", all.len()))
        }
    }
}

/// Execute a plugin manager task.
pub async fn run(task: Task, svc: UiServices) -> Input {
    match task {
        Task::Load => Input::Loaded(rows(&svc)),
        Task::Versions(id) => Input::Versions(
            svc.plugins
                .host
                .available_versions(&id)
                .await
                .into_iter()
                .map(|v| v.version.to_string())
                .collect(),
        ),
        Task::Op(o) => op(o, &svc).await,
        Task::SaveSetting {
            plugin_id,
            key,
            value,
        } => match svc
            .settings
            .set_value(&setting_key(&plugin_id, &key), value)
        {
            Ok(()) => Input::Versions(
                svc.plugins
                    .host
                    .available_versions(&plugin_id)
                    .await
                    .into_iter()
                    .map(|v| v.version.to_string())
                    .collect(),
            ),
            Err(e) => Input::Done(false, e.to_user_message()),
        },
    }
}
