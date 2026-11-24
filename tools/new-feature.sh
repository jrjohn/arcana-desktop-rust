#!/usr/bin/env bash
# Add one business feature as a vertical slice through every layer, mirroring Customer/Product.
#
#   tools/new-feature.sh Invoice
#
# Generates (P = the crate directory prefix, `arcana`; Rust paths use the package prefix):
#   domain  crates/P-domain/src/entities/<x>.rs           entity (BaseEntity + code/name/is_active)
#           crates/P-domain/src/services/<x>.rs           <X>Service trait
#   data    crates/P-data/migrations/<ts>_<x>.sql         table "<Xs>"
#           crates/P-data/src/tables.rs                   base_table! mapping (appended)
#           crates/P-data/src/repository/<x>.rs           <X>Repository trait + <X>RepositoryImpl
#           crates/P-data/tests/<x>_repository_tests.rs
#   infra   crates/P-infra/src/services/<x>.rs            <X>ServiceImpl
#           crates/P-infra/src/wiring.rs                  ArcanaServices.<xs> (composition root)
#           crates/P-infra/tests/<x>_service_tests.rs
#   ui      crates/P-ui/src/pages/<x>/{mod,task,view}.rs  view model (Input → Effect), task, view
#           crates/P-ui/src/services.rs                   UiServices.<xs>
# then runs cargo fmt. Every edit of an existing file is anchored on a line that must occur
# exactly once; a missing anchor aborts before anything is written (AGENTS.md §1).
set -euo pipefail

[ $# -eq 1 ] || { echo "usage: $0 <FeatureName in PascalCase, e.g. Invoice>" >&2; exit 1; }
NAME="$1"
[[ "$NAME" =~ ^[A-Z][a-z0-9]+([A-Z][a-z0-9]+)*$ ]] || { echo "feature name must be PascalCase (Invoice, PurchaseOrder)" >&2; exit 1; }
command -v perl >/dev/null || { echo "perl is required" >&2; exit 1; }

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

DOMAIN_DIR="$(ls -d crates/*-domain 2>/dev/null | head -1)"
[ -n "$DOMAIN_DIR" ] || { echo "new-feature: no crates/*-domain crate — is this an arcana-desktop-rust workspace?" >&2; exit 1; }
P="${DOMAIN_DIR#crates/}"; P="${P%-domain}"          # crate DIRECTORY prefix (always arcana)
# Package prefix from the domain crate's Cargo name: tools/new-app.sh renames packages
# (arcana-domain -> inventory-domain) but keeps the crates/arcana-*/ directories.
PKG="$(perl -ne 'print "$1" and exit if /^name\s*=\s*"(.+)-domain"/' "$DOMAIN_DIR/Cargo.toml")"
[ -n "$PKG" ] || { echo "new-feature: cannot read the package name from $DOMAIN_DIR/Cargo.toml" >&2; exit 1; }
PU="$(printf '%s' "$PKG" | tr '-' '_')"              # Rust path prefix, e.g. arcana / inventory

snake="$(printf '%s' "$NAME" | perl -pe 's/(?<!^)([A-Z])/_\l$1/g; $_=lcfirst')"
case "$NAME" in
    *[^aeiou]y) PLURAL="${NAME%y}ies" ;;
    *s|*x|*ch|*sh) PLURAL="${NAME}es" ;;
    *) PLURAL="${NAME}s" ;;
esac
snakes="$(printf '%s' "$PLURAL" | perl -pe 's/(?<!^)([A-Z])/_\l$1/g; $_=lcfirst')"
TITLE="$(printf '%s' "$PLURAL" | perl -pe 's/(?<!^)([A-Z])/ $1/g')"

D=crates/$P-domain; DA=crates/$P-data; I=crates/$P-infra; U=crates/$P-ui
for f in "$D/src/entities/mod.rs" "$D/src/services.rs" "$DA/src/tables.rs" "$DA/src/repository/mod.rs" \
         "$DA/src/lib.rs" "$DA/src/db.rs" "$I/src/services.rs" "$I/src/wiring.rs" "$I/src/lib.rs" \
         "$U/src/pages/mod.rs" "$U/src/services.rs"; do
    [ -f "$f" ] || { echo "new-feature: expected file $f is missing — layout changed; update tools/new-feature.sh" >&2; exit 1; }
done
for f in "$D/src/entities/$snake.rs" "$D/src/services/$snake.rs" "$DA/src/repository/$snake.rs" \
         "$I/src/services/$snake.rs" "$U/src/pages/$snake"; do
    [ ! -e "$f" ] || { echo "new-feature: $f already exists — refusing to overwrite" >&2; exit 1; }
done
grep -q "\"$PLURAL\"" "$DA/src/tables.rs" && { echo "new-feature: table \"$PLURAL\" already mapped" >&2; exit 1; }

# anchor <file> <exact line> — the line must occur exactly once (checked for ALL edits first).
ANCHORS=()
need() {
    local n; n="$(grep -cxF -- "$2" "$1" || true)"
    [ "$n" = 1 ] || { echo "new-feature: anchor not found exactly once ($n×) in $1: '$2' — update tools/new-feature.sh" >&2; exit 1; }
}
# after <file> <anchor line> <text to insert after it>
after() {
    A="$2" T="$3" perl -0pi -e 's/^\Q$ENV{A}\E\n/$&$ENV{T}\n/m' "$1"
}
need "$D/src/entities/mod.rs" "mod identity;"
need "$D/src/entities/mod.rs" "pub use base::{BaseEntity, BaseGuidEntity, HasBase};"
need "$DA/src/repository/mod.rs" "mod specialized;"
need "$DA/src/lib.rs" "pub use row::{FromSql, SqlValue, TableEntity, ToSql};"
need "$DA/src/db.rs" "                \"AuditLogs\","
need "$I/src/wiring.rs" "    pub orders: Arc<dyn OrderService>,"
need "$I/src/wiring.rs" "            settings: Arc::new(JsonSettingsService::new(config.settings_path)),"
need "$I/src/wiring.rs" "use crate::services::{CustomerServiceImpl, OrderServiceImpl, ProductServiceImpl};"
need "$I/src/lib.rs" "pub use services::{CustomerServiceImpl, OrderServiceImpl, ProductServiceImpl, SharedValidator};"
need "$U/src/pages/mod.rs" "pub mod settings;"
need "$U/src/services.rs" "    pub products: Arc<dyn ProductService>,"
need "$U/src/services.rs" "            products: s.products.clone(),"

# render <dest> — template on stdin with placeholders substituted
render() {
    mkdir -p "$(dirname "$1")"
    perl -pe "s/__P__/$PU/g; s/__Name__/$NAME/g; s/__Names__/$PLURAL/g; s/__name__/$snake/g; s/__names__/$snakes/g; s/__Title__/$TITLE/g" > "$1"
    echo "  + $1"
}

echo "new-feature: $NAME (packages ${PKG}-*, directories crates/$P-*)"

# ---------------------------------------------------------------- domain: entity
render "$D/src/entities/$snake.rs" <<'EOF'
//! `__Name__` entity (generated by tools/new-feature.sh — extend the fields, keep `base`).

use super::base::{BaseEntity, impl_base_entity};

/// A __name__ record. Persisted columns are mapped in `__P___data::tables`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct __Name__ {
    pub base: BaseEntity,
    pub code: String,
    pub name: String,
    pub is_active: bool,
}

impl Default for __Name__ {
    fn default() -> Self {
        Self {
            base: BaseEntity::default(),
            code: String::new(),
            name: String::new(),
            is_active: true,
        }
    }
}

impl_base_entity!(__Name__);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::HasBase;

    #[test]
    fn default_is_active_and_unsaved() {
        let e = __Name__::default();
        assert!(e.is_active);
        assert_eq!(e.base().id, 0);
    }
}
EOF
after "$D/src/entities/mod.rs" "mod identity;" "mod $snake;"
after "$D/src/entities/mod.rs" "pub use base::{BaseEntity, BaseGuidEntity, HasBase};" "pub use $snake::$NAME;"

# ---------------------------------------------------------------- domain: service trait
render "$D/src/services/$snake.rs" <<'EOF'
//! `__Name__Service` contract. Implementation: `__P___infra::services::__name__::__Name__ServiceImpl`,
//! wired in `__P___infra::wiring` (the only place that names it).

use __P___core::{AppResult, PageRequest, PagedResult};
use async_trait::async_trait;

use crate::entities::__Name__;

#[async_trait]
pub trait __Name__Service: Send + Sync {
    async fn get___names__(&self, request: &PageRequest) -> AppResult<PagedResult<__Name__>>;
    async fn get___name___by_id(&self, id: i32) -> AppResult<__Name__>;
    async fn create___name__(&self, item: __Name__) -> AppResult<__Name__>;
    async fn update___name__(&self, item: __Name__) -> AppResult<__Name__>;
    async fn delete___name__(&self, id: i32) -> AppResult<()>;
}
EOF
printf '\npub mod %s;\npub use %s::%sService;\n' "$snake" "$snake" "$NAME" >> "$D/src/services.rs"

# ---------------------------------------------------------------- data: migration + mapping
TS="$(date -u +%Y%m%d%H%M%S)"
render "$DA/migrations/${TS}_${snake}.sql" <<'EOF'
-- __Names__ (tools/new-feature.sh). A schema change is a NEW migration file; never edit an applied one.
CREATE TABLE "__Names__" (
    "Id" INTEGER NOT NULL CONSTRAINT "PK___Names__" PRIMARY KEY AUTOINCREMENT,
    "Code" TEXT NOT NULL,
    "Name" TEXT NOT NULL,
    "IsActive" INTEGER NOT NULL,
    "CreatedAt" TEXT NOT NULL,
    "CreatedBy" TEXT NULL,
    "ModifiedAt" TEXT NULL,
    "ModifiedBy" TEXT NULL,
    "IsDeleted" INTEGER NOT NULL,
    "DeletedAt" TEXT NULL,
    "DeletedBy" TEXT NULL,
    "SyncId" TEXT NOT NULL,
    "LastSyncAt" TEXT NULL,
    "IsPendingSync" INTEGER NOT NULL
);

CREATE UNIQUE INDEX "IX___Names___Code" ON "__Names__" ("Code");
EOF
cat >> "$DA/src/tables.rs" <<EOF

base_table!(${PU}_domain::$NAME, "$PLURAL", filter = true, {
    "Code" => code, "Name" => name, "IsActive" => is_active,
});
EOF
# db.rs asserts the full, sorted table list: insert the new table in order.
T="$PLURAL" perl -0pi -e '
    s{\n([ \t]*)"AuditLogs",\n((?:[ \t]*"[A-Za-z]+",?\n)*)}{
        my $ind=$1; my @n = ("AuditLogs", ($2 =~ /"([A-Za-z]+)"/g), $ENV{T}); @n = sort @n;
        "\n" . join(",\n", map { "$ind\"$_\"" } @n) . "\n"
    }e' "$DA/src/db.rs"
grep -qE "^[[:space:]]*\"$PLURAL\",?$" "$DA/src/db.rs" || { echo "new-feature: could not add \"$PLURAL\" to the table list in $DA/src/db.rs" >&2; exit 1; }

# ---------------------------------------------------------------- data: repository
render "$DA/src/repository/$snake.rs" <<'EOF'
//! `__Name__Repository` + its `SQLite` implementation (same shape as `CustomerRepositoryImpl`).

use __P___core::{AppResult, Repository};
use __P___domain::__Name__;
use async_trait::async_trait;

use super::generic::{SqliteRepository, delegate_repository};
use crate::db::Db;

#[async_trait]
pub trait __Name__Repository: Repository<__Name__> {
    async fn get_by_code(&self, code: &str) -> AppResult<Option<__Name__>>;
}

#[derive(Clone)]
pub struct __Name__RepositoryImpl {
    base: SqliteRepository<__Name__>,
}

impl __Name__RepositoryImpl {
    #[must_use]
    pub fn with_db(db: Db) -> Self {
        Self {
            base: SqliteRepository::new(db),
        }
    }

    async fn add_impl(&self, e: __Name__) -> AppResult<__Name__> {
        self.base.add(e).await
    }
    async fn add_range_impl(&self, e: Vec<__Name__>) -> AppResult<()> {
        self.base.add_range(e).await
    }
    async fn update_impl(&self, e: &mut __Name__) -> AppResult<()> {
        self.base.update(e).await
    }
}

delegate_repository!(__Name__RepositoryImpl, __Name__);

#[async_trait]
impl __Name__Repository for __Name__RepositoryImpl {
    async fn get_by_code(&self, code: &str) -> AppResult<Option<__Name__>> {
        self.base.first_or_default(&|e: &__Name__| e.code == code).await
    }
}
EOF
after "$DA/src/repository/mod.rs" "mod specialized;" "mod $snake;
pub use $snake::{${NAME}Repository, ${NAME}RepositoryImpl};"
after "$DA/src/lib.rs" "pub use row::{FromSql, SqlValue, TableEntity, ToSql};" "pub use repository::{${NAME}Repository, ${NAME}RepositoryImpl};"

render "$DA/tests/${snake}_repository_tests.rs" <<'EOF'
//! `__Name__RepositoryImpl` on an in-memory database (generated by tools/new-feature.sh).

use __P___core::Repository;
use __P___data::{Db, __Name__Repository, __Name__RepositoryImpl};
use __P___domain::__Name__;

fn item(code: &str) -> __Name__ {
    __Name__ {
        code: code.into(),
        name: format!("{code} name"),
        ..__Name__::default()
    }
}

#[tokio::test]
async fn add_then_find_by_code_and_soft_delete() {
    let repo = __Name__RepositoryImpl::with_db(Db::in_memory().await.unwrap());
    let mut saved = repo.add(item("A-1")).await.unwrap();
    assert!(saved.base.id > 0);
    assert_eq!(repo.get_by_code("A-1").await.unwrap().unwrap().name, "A-1 name");
    assert!(repo.get_by_code("missing").await.unwrap().is_none());
    repo.delete(&mut saved).await.unwrap();
    assert!(repo.get_by_id(saved.base.id).await.unwrap().is_none());
}
EOF

# ---------------------------------------------------------------- infra: service impl + wiring
render "$I/src/services/$snake.rs" <<'EOF'
//! `__Name__ServiceImpl`: business rules for __names__ over `__Name__Repository`.

use std::sync::Arc;

use __P___core::{AppError, AppResult, PageRequest, PagedResult};
use __P___data::__Name__Repository;
use __P___domain::__Name__;
use __P___domain::services::__Name__Service;
use async_trait::async_trait;

use super::{not_found, read, write};

pub struct __Name__ServiceImpl {
    repo: Arc<dyn __Name__Repository>,
}

impl __Name__ServiceImpl {
    #[must_use]
    pub fn new(repo: Arc<dyn __Name__Repository>) -> Self {
        Self { repo }
    }

    fn validate(item: &__Name__) -> AppResult<()> {
        let mut errors = Vec::new();
        if item.code.trim().is_empty() {
            errors.push("Code is required".to_owned());
        }
        if item.name.trim().is_empty() {
            errors.push("Name is required".to_owned());
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(AppError::validation("__Name__ validation failed", errors))
        }
    }
}

#[async_trait]
impl __Name__Service for __Name__ServiceImpl {
    async fn get___names__(&self, request: &PageRequest) -> AppResult<PagedResult<__Name__>> {
        read(self.repo.get_paged(request).await, "Failed to get __names__")
    }

    async fn get___name___by_id(&self, id: i32) -> AppResult<__Name__> {
        read(self.repo.get_by_id(id).await, "Failed to get __name__")?
            .ok_or_else(|| not_found(format!("__Name__ with ID {id} not found")))
    }

    async fn create___name__(&self, item: __Name__) -> AppResult<__Name__> {
        Self::validate(&item)?;
        write(self.repo.add(item).await, "Failed to create __name__")
    }

    async fn update___name__(&self, mut item: __Name__) -> AppResult<__Name__> {
        const MSG: &str = "Failed to update __name__";
        let id = item.base.id;
        if write(self.repo.get_by_id(id).await, MSG)?.is_none() {
            return Err(not_found(format!("__Name__ with ID {id} not found")));
        }
        Self::validate(&item)?;
        write(self.repo.update(&mut item).await, MSG)?;
        Ok(item)
    }

    async fn delete___name__(&self, id: i32) -> AppResult<()> {
        const MSG: &str = "Failed to delete __name__";
        let mut item = write(self.repo.get_by_id(id).await, MSG)?
            .ok_or_else(|| not_found(format!("__Name__ with ID {id} not found")))?;
        write(self.repo.delete(&mut item).await, MSG)
    }
}
EOF
printf '\npub mod %s;\n' "$snake" >> "$I/src/services.rs"
after "$I/src/lib.rs" "pub use services::{CustomerServiceImpl, OrderServiceImpl, ProductServiceImpl, SharedValidator};" "pub use services::$snake::${NAME}ServiceImpl;"
after "$I/src/wiring.rs" "use crate::services::{CustomerServiceImpl, OrderServiceImpl, ProductServiceImpl};" "use crate::services::$snake::${NAME}ServiceImpl;
use ${PU}_data::${NAME}RepositoryImpl;
use ${PU}_domain::services::${NAME}Service;"
after "$I/src/wiring.rs" "    pub orders: Arc<dyn OrderService>," "    pub $snakes: Arc<dyn ${NAME}Service>,"
A="            settings: Arc::new(JsonSettingsService::new(config.settings_path))," \
T="            $snakes: Arc::new(${NAME}ServiceImpl::new(Arc::new(${NAME}RepositoryImpl::with_db(db.clone())))),"
A="$A" T="$T" perl -0pi -e 's/^\Q$ENV{A}\E\n/$ENV{T}\n$&/m' "$I/src/wiring.rs"

render "$I/tests/${snake}_service_tests.rs" <<'EOF'
//! `__Name__Service` through the composition root on an in-memory database
//! (generated by tools/new-feature.sh).

use std::sync::Arc;

use __P___core::{ErrorCode, PageRequest};
use __P___data::{Db, __Name__RepositoryImpl};
use __P___domain::__Name__;
use __P___domain::services::__Name__Service;
use __P___infra::__Name__ServiceImpl;

async fn svc() -> __Name__ServiceImpl {
    let db = Db::in_memory().await.unwrap();
    __Name__ServiceImpl::new(Arc::new(__Name__RepositoryImpl::with_db(db)))
}

fn item(code: &str) -> __Name__ {
    __Name__ {
        code: code.into(),
        name: "Sample".into(),
        ..__Name__::default()
    }
}

#[tokio::test]
async fn create_get_update_delete() {
    let s = svc().await;
    let created = s.create___name__(item("C-1")).await.unwrap();
    let id = created.base.id;
    assert_eq!(s.get___name___by_id(id).await.unwrap().code, "C-1");
    let mut changed = created.clone();
    changed.name = "Renamed".into();
    assert_eq!(s.update___name__(changed).await.unwrap().name, "Renamed");
    assert_eq!(s.get___names__(&PageRequest::default()).await.unwrap().items.len(), 1);
    s.delete___name__(id).await.unwrap();
    assert_eq!(s.get___name___by_id(id).await.unwrap_err().code(), ErrorCode::NotFound);
}

#[tokio::test]
async fn create_rejects_blank_code() {
    let err = svc().await.create___name__(item(" ")).await.unwrap_err();
    assert_eq!(err.code(), ErrorCode::ValidationFailed);
}
EOF

# ---------------------------------------------------------------- ui: view model, task, view
render "$U/src/pages/$snake/mod.rs" <<'EOF'
//! __Title__ list page: view model (state + `on(Input) -> Vec<Effect>`, no egui, no I/O),
//! task (async service calls) and view (draws state, returns inputs). Same shape as `home`.
//!
//! To show it in the shell, route it like `ProductList`: a `Route` variant + `NavGraph::to_*`
//! in `nav.rs`, and `Page` / `PageMsg` / `PageTask` arms in `pages/mod.rs`.

mod task;
mod view;

pub use task::run;
pub use view::show;

use __P___domain::__Name__;

use crate::effect::{Effect, Notice};

/// Rows loaded per request.
pub const PAGE_SIZE: u32 = 50;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct State {
    pub items: Vec<__Name__>,
    pub loading: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    Load,
    Loaded(Result<Vec<__Name__>, String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Task {
    LoadAll,
}

#[derive(Debug, Default)]
pub struct ViewModel {
    pub state: State,
}

impl ViewModel {
    /// New view model plus its initial effects (load).
    #[must_use]
    pub fn new() -> (Self, Vec<Effect<Task>>) {
        let mut vm = Self::default();
        let fx = vm.on(Input::Load);
        (vm, fx)
    }

    pub fn on(&mut self, input: Input) -> Vec<Effect<Task>> {
        let s = &mut self.state;
        match input {
            Input::Load => {
                if s.loading {
                    return vec![];
                }
                s.loading = true;
                s.error = None;
                vec![Effect::Run(Task::LoadAll)]
            }
            Input::Loaded(Ok(items)) => {
                s.loading = false;
                s.items = items;
                vec![]
            }
            Input::Loaded(Err(e)) => {
                s.loading = false;
                s.error = Some(e.clone());
                vec![Effect::Notify(Notice::error(e))]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_loads_once() {
        let (mut vm, fx) = ViewModel::new();
        assert_eq!(fx, vec![Effect::Run(Task::LoadAll)]);
        assert!(vm.state.loading);
        assert!(vm.on(Input::Load).is_empty(), "no second load while loading");
    }

    #[test]
    fn loaded_fills_state_and_error_notifies() {
        let (mut vm, _) = ViewModel::new();
        vm.on(Input::Loaded(Ok(vec![__Name__::default()])));
        assert_eq!(vm.state.items.len(), 1);
        assert!(!vm.state.loading);
        let fx = vm.on(Input::Loaded(Err("boom".into())));
        assert_eq!(fx, vec![Effect::Notify(Notice::error("boom"))]);
        assert_eq!(vm.state.error.as_deref(), Some("boom"));
    }
}
EOF
render "$U/src/pages/$snake/task.rs" <<'EOF'
use __P___core::PageRequest;

use super::{Input, PAGE_SIZE, Task};
use crate::services::UiServices;

/// Execute a __name__ task.
pub async fn run(task: Task, svc: UiServices) -> Input {
    match task {
        Task::LoadAll => Input::Loaded(
            svc.__names__
                .get___names__(&PageRequest::new(1, PAGE_SIZE))
                .await
                .map(|page| page.items)
                .map_err(|e| e.to_user_message()),
        ),
    }
}
EOF
render "$U/src/pages/$snake/view.rs" <<'EOF'
use egui::Ui;

use super::{Input, State};
use crate::widgets::{ViewCx, card, page_title};

/// Draw the __name__ list; returns the inputs the user produced. Reads state only.
pub fn show(ui: &mut Ui, state: &State, cx: &ViewCx<'_>) -> Vec<Input> {
    let mut out = Vec::new();
    page_title(ui, "__Title__", None);
    if ui.button("⟳").clicked() {
        out.push(Input::Load);
    }
    if let Some(e) = &state.error {
        ui.colored_label(egui::Color32::RED, e);
    }
    card(ui, cx, |ui| {
        for item in &state.items {
            ui.label(format!("{}  {}", item.code, item.name));
        }
    });
    out
}
EOF
after "$U/src/pages/mod.rs" "pub mod settings;" "pub mod $snake;"
after "$U/src/services.rs" "    pub products: Arc<dyn ProductService>," "    pub $snakes: Arc<dyn ${PU}_domain::services::${NAME}Service>,"
after "$U/src/services.rs" "            products: s.products.clone()," "            $snakes: s.$snakes.clone(),"

cargo fmt --all
echo "new-feature: $NAME generated. Next: cargo xtask layers && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && tools/arch-check.sh"
echo "             then route the page (see crates/$P-ui/src/pages/$snake/mod.rs header)."
