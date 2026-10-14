# arcana-desktop-rust 移植計畫(arcana-windows → Rust + egui)

> 2026-10-14 · 來源 `jrjohn/arcana-windows`(net10 WinUI 3)→ 本 repo
> 參考:storytold/photocraft(分層 + WASM 外掛範本)、storytold/artcraft(反例:無外掛)

## 0. 原專案現況(盤點結果)
- 8 個專案約 2.3 萬行,FlowChart 外掛 5.4 千行,測試約 1.6 萬行(約 949 個測試)。
- 依賴:App → Infrastructure → {Sync → Data → Domain → Core};Plugins → Plugins.Contracts。
- **規格多於實作**:`WINDOWS_APP_AI_PROMPT.md`(1.2 萬行)寫的 Marketplace、When-expression 多半沒做。
- **Sync 只有 CRDT 資料結構**(LWWRegister/LWWMap/MVRegister/VectorClock),沒有 SyncService 實作、沒有伺服器 → 傳輸要從零設計。
- **外掛權限只是程式內檢查**(PluginPermission flags),不是真沙箱。
- 沒有 HTTP API、沒有 migration(`EnsureCreatedAsync`)、沒有登入頁、沒有 DB 加密。

移植原則:**照「實作」一比一,不照「規格」**;規格裡沒做的列 backlog。

## 1. C# → Rust crate 對照

| C# 專案 | Rust crate | 內容 | 主要套件 |
|---|---|---|---|
| Arcana.Core | `arcana-core` | Result/AppError、Entity、Repository/UnitOfWork trait、PagedResult、Security trait | thiserror |
| Arcana.Domain | `arcana-domain` | Customer/Product/ProductCategory/Order/OrderItem、Identity 7 實體、驗證器、Service trait | — |
| Arcana.Data | `arcana-data` | SQLite(13 表 + 外掛 3 表)、DAO + Repository、**正式 migration** | sqlx(sqlite) |
| Arcana.Sync | `arcana-sync` | VectorClock、LWWRegister/LWWMap、MVRegister、5 種衝突策略、SyncQueue;傳輸 trait | serde |
| Arcana.Plugins.Contracts | `arcana-plugin-api` | 18 種 PluginType、`Plugin` trait、PluginContext、manifest | serde |
| Arcana.Plugins | `arcana-plugin-host` | 內建 inventory 登錄 + WASM(wasmtime)、拓撲 + SemVer、權限、健康監控、MessageBus | wasmtime、semver |
| Arcana.Infrastructure | `arcana-infra` | Service 實作、PBKDF2 + HMAC token、RBAC + AuditLog、設定、i18n(en-US/zh-TW/ja-JP)、log | pbkdf2、hmac、tracing |
| Arcana.App | `arcana-ui` + `arcana-app`(bin) | egui 外殼、分頁文件 + 浮動視窗、NavGraph、各頁 ViewModel | eframe/egui、egui_dock |
| plugins/FlowChartModule | `plugins/flowchart` | Diagram/Node/Edge、自繪畫布、snap、undo/redo、draw.io mxGraph XML | quick-xml、flate2 |
| (新增) | `xtask` | `cargo xtask layers` 強制依賴方向(借 photocraft) | — |

依賴方向(CI 由 xtask 擋):`core ← domain ← data ← sync ← infra ← ui ← app`;`plugin-api` 只依賴 core;**ui 以下不准依賴 egui**。

## 2. 關鍵設計
- **MVVM-UDF**:每頁 `ViewModel { state, fn on(Input) -> Vec<Effect> }`,Input 是強型別 enum;View 只讀 state、只送 Input。
- **單一 Command registry**:UI、選單、快捷鍵、外掛(未來 CLI/MCP)走同一條入口。
- **op-based 變更**:每個 Command 產生可逆 op → 同一份 op 餵 undo 與 CRDT 同步。
- **外掛兩層**:內建(Order/Product/Customer/CoreMenu/System/FlowChart)用 `Plugin` trait + inventory;第三方用 WASM component(WIT),預設零 host import、權限宣告才給、fuel/記憶體/時間上限、交易式套用;manifest 參數 schema → 自動產生設定 UI。WASM 外掛以**宣告式 view 描述**提供畫面,由 host 繪製。
- **多分頁 + 浮動視窗**:egui_dock;浮動用 egui 多 viewport。

## 3. 里程碑(每段一個 draft PR)
1. **骨架 + CI**:workspace + xtask layers、Jenkinsfile、Sonar、clippy pedantic -D warnings、空殼視窗。
2. **core + domain + data**:實體、驗證、SQLite migration、Repository;移植 Domain 86 + Data 9 測試。
3. **infra + sync**:安全/RBAC/AuditLog、i18n、設定;CRDT 全部 + Sync 120 測試;傳輸先 trait + 假實作。
4. **egui UI**:外殼、dock、NavGraph、Home/Order/Product/Customer/Settings/PluginManager;Infra 255 測試;egui_kittest。
5. **外掛 + FlowChart**:plugin-host、WASM 沙箱、FlowChart 畫布 + draw.io;Plugins 452 + FlowChart 27 測試;對照 `docs/screen/*.png`。

估時約 20–29 人週;最難:外掛 UI 協定、多視窗、FlowChart 互動、同步協定。

## 4. 不在本次範圍(原版也沒做)
Marketplace、When-expression、同步伺服器、DB 加密、登入頁。

## 5. 驗收
- 每段:`cargo test` 全綠、clippy 0 warning、`cargo xtask layers` 綠、Sonar 閘綠、arch-qube 過。
- 最終:原 949 測試都有對應版本;5 張截圖畫面逐一功能對照;Windows/macOS/Linux 可執行。

## 6. 里程碑 3 紀錄(infra + sync)
- **測試**:Sync 120/120、Infrastructure 255/255 一對一移植(註解標 C# 測試名);另加 CRDT proptest 收斂性質、`QueueSyncService` 兩副本經 loopback 收斂/離線重試、設定/語系/日誌/wiring 端到端。延到 M5:無(C# Infra 測試沒有碰外掛 runtime 的)。
- **欄位層級合併**:C# 靠反射 + `{Name}ModifiedAt` 慣例 + `[MergeStrategy]/[LocalOnly]/[RemoteOnly]` 屬性(但 `ResolveFieldLevel` 從沒讀那些屬性)。Rust 用 `Mergeable::fields()` 明列 `FieldDescriptor`(`field!` 巨集),`FieldPolicy` 的語意**有真的生效**;未列出的欄位取本地值。
- **SyncService**:C# 只有介面。新增 `QueueSyncService`(SyncQueue 表 + `SyncTransport` trait + `ChangeApplier`),測試用 `LoopbackHub` 記憶體中繼;未定義任何網路協定。
- **刻意偏離 C#**:PasswordHasher 空字串回 `AppError::Validation` 而非拋例外(core trait 改回傳 `AppResult`);TokenService 金鑰一律注入(拿掉 C# 以機器名+使用者名推導的可猜金鑰);`VectorClock` 的 hash 忽略 0 值項目以符合「因果相等」;`IComparable` 對 Concurrent 回 0 → Rust `PartialOrd` 回 `None`;`MvRegister::single_value` 回 `AppResult`;C# 沒實作的 key/value `SettingsService` 補 `JsonSettingsService`。
- **保留的 C# 怪行為**:沒有截止時間的鎖定在下次登入時視同過期而自動解鎖;`AuthService` 算權限時不排除過期的角色指派(`AuthorizationService` 會排除);`KeepBoth` 與沒註冊 resolver 的 `Custom` 都退回 LWW。
- **延後**:外掛相關(`AddPluginServices`)→ M5;`NetworkMonitorImpl`(.NET `NetworkInterface`,無可攜 std 對應)→ M4 與 UI 外殼一起決定;`LocalizationService` 契約暫放 `arcana-infra`,M5 移到 `arcana-plugin-api`。
