//! Renders each main page of the real shell offscreen (wgpu) and writes PNGs for side-by-side
//! comparison with the C# screenshots.
//!
//! `cargo run -p arcana-app --features screenshots --bin screenshots -- docs/screen-rust`

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use arcana_infra::network::{ProbeNetworkMonitor, StaticProbe};
use arcana_infra::wiring::ArcanaServices;
use arcana_ui::commands::CommandId;
use arcana_ui::pages::settings::Input as S;
use arcana_ui::pages::{Page, PageMsg, order_list, plugin_manager};
use arcana_ui::{Shell, UiServices, fonts};
use egui_kittest::Harness;

#[path = "../bootstrap.rs"]
#[allow(dead_code)]
mod bootstrap;

fn settle(h: &mut Harness<'_, Shell>) {
    for _ in 0..3 {
        h.run_steps(3);
        h.state_mut().settle(Duration::from_secs(10));
    }
    h.run_steps(3);
}

fn save(
    h: &mut Harness<'_, Shell>,
    dir: &Path,
    name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    settle(h);
    let img = h.render()?;
    let path = dir.join(format!("{name}.png"));
    img.save(&path)?;
    println!("wrote {}", path.display());
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "docs/screen-rust".to_owned()),
    );
    std::fs::create_dir_all(&dir)?;
    let tmp = std::env::temp_dir().join(format!("arcana-shots-{}", std::process::id()));
    std::fs::create_dir_all(&tmp)?;
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let svc = rt.block_on(async {
        let s = ArcanaServices::in_memory(bootstrap::config(&tmp)).await?;
        arcana_infra::seed::seed_demo_data(&s).await?;
        Ok::<_, arcana_infra::AppError>(s)
    })?;
    let ui = UiServices::from_arcana(
        &svc,
        Arc::new(ProbeNetworkMonitor::new(StaticProbe(true))),
        &tmp,
    );
    ui.app_settings.set_language_code("en-US");
    ui.app_settings.set_theme_id("ForestGreen");
    let shell = Shell::new(ui, rt.handle().clone());
    let mut h = Harness::builder()
        .with_size(egui::vec2(1260.0, 800.0))
        .with_pixels_per_point(1.0)
        .wgpu()
        .build_ui_state(|ui, shell: &mut Shell| shell.show(ui), shell);
    fonts::install(&h.ctx);

    save(&mut h, &dir, "home")?;

    h.state_mut().execute(CommandId::OrderList);
    save(&mut h, &dir, "order-list")?;

    let list = h.state().focused_tab().ok_or("no tab")?;
    let first = match h.state_mut().page_mut(list) {
        Some(Page::OrderList(vm)) => vm.state.orders.first().map(|o| o.base.id),
        _ => None,
    }
    .ok_or("no orders")?;
    h.state_mut().send(
        list,
        PageMsg::OrderList(order_list::Input::EditOrder(first)),
    );
    save(&mut h, &dir, "order-detail")?;

    h.state_mut().execute(CommandId::NewOrder);
    save(&mut h, &dir, "order-new")?;

    h.state_mut().execute(CommandId::ProductList);
    save(&mut h, &dir, "product-list")?;

    h.state_mut().execute(CommandId::CustomerList);
    save(&mut h, &dir, "customer-list")?;

    let products = h
        .state()
        .find_tab(&arcana_ui::nav::Route::ProductList)
        .ok_or("no products")?;
    if let Some(Page::ProductList(vm)) = h.state_mut().page_mut(products) {
        let id = vm
            .state
            .rows
            .first()
            .map(|p| p.base.id)
            .ok_or("no product")?;
        h.state_mut()
            .navigate(&arcana_ui::nav::NavGraph::to_product_detail(id), None);
    }
    save(&mut h, &dir, "product-detail")?;

    h.state_mut().execute(CommandId::PluginManager);
    let pm = h.state().focused_tab().ok_or("no tab")?;
    h.state_mut().send(
        pm,
        PageMsg::PluginManager(plugin_manager::Input::Select("arcana.module.order".into())),
    );
    h.state_mut().send(
        pm,
        PageMsg::PluginManager(plugin_manager::Input::CheckAllHealth),
    );
    save(&mut h, &dir, "plugin-manager")?;

    // FlowChart plugin: Tools → FlowChart → Sample FlowChart opens the editor tab.
    h.state_mut().run_plugin_command("flowchart.sample");
    save(&mut h, &dir, "flowchart")?;

    h.state_mut().execute(CommandId::Settings);
    save(&mut h, &dir, "settings")?;

    // Live language + theme switch through the settings page.
    let st = h.state().focused_tab().ok_or("no tab")?;
    h.state_mut()
        .send(st, PageMsg::Settings(S::SelectLanguage("zh-TW".into())));
    h.state_mut()
        .send(st, PageMsg::Settings(S::SelectTheme("Dark".into())));
    save(&mut h, &dir, "settings-zh-TW-dark")?;
    h.state_mut().execute(CommandId::Home);
    save(&mut h, &dir, "home-zh-TW-dark")?;

    let _ = std::fs::remove_dir_all(&tmp);
    Ok(())
}
