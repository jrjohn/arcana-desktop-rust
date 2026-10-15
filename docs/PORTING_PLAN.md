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

## 7. 里程碑 4 紀錄(egui UI)
- **結構**:`arcana-ui` = `pages/*`(每頁 `ViewModel { state, on(Input) -> Vec<Effect> }` + `view.rs` + `task.rs`)、`shell/*`(選單、側欄、`egui_dock` 分頁、浮動視窗、狀態列、對話框、toast)、`commands`(單一 Command registry:選單 / 快捷鍵 / 工具列 / 命令搜尋都走 `Shell::execute`)、`nav`(C# `NavGraph.Routes` 一字不差)、`runtime`(tokio 執行 task,結果經 channel 回成 Input)。VM 不碰 egui、不做 I/O,`Effect::Run(Task)` 由 shell 交給 executor。
- **分層表變更**(xtask):`arcana-ui → arcana-domain`(VM 直接操作實體與驗證器);`arcana-app → arcana-infra`(組裝根要接 DB 與服務)。GUI 套件清單加 `egui_dock`、`egui_kittest`。
- **頁面對照 C#**:Home(C# 卡片是寫死的 12 / $45,600 / 5 / 128 且不載最近訂單 → 改為實算)、OrderList(C# 有搜尋/狀態/日期欄位但查詢時沒帶上、欄位不能排序 → 都接上)、OrderDetail/新增訂單(欄位、品項格、合計、Copy/Edit/Save/Cancel/Print 同 C#;存檔前跑 domain `OrderValidator` 並顯示訊息)、Settings(9 個主題 + 強調色、語系即時切換、同步設定、C# TODO 的按鈕改顯示「開發中」)、PluginManager(列 C# 啟動時註冊的 5 個內建外掛;安裝/啟用/停用/重載/升級/回滾/解除安裝需 M5 runtime,灰掉並註明)。**ProductList/Detail、CustomerList/Detail 在 C# 只有「開發中」佔位頁**,這裡補了可用的清單 + 表單(泛型 `entity_list` / `entity_form`)。
- **刻意偏離**:C# 每次啟動 `EnsureDeleted` 重建 DB;這裡只在空 DB 時 seed(`arcana_infra::seed`,同樣 5 客戶 / 3 類別 / 8 產品 / 10 訂單,但亂數用自訂 LCG 而非 .NET `Random(42)`)。C# 巢狀分頁(模組頁內再一層 DocumentTabHost)改為單層 dock;浮動視窗用 egui 多 viewport,不支援時(kittest)自動退回內嵌視窗。
- **保留的 C# 怪行為**:更新訂單時被移除的品項不會從 DB 刪除(M2 repository 照 EF 行為)。
- **NetworkMonitorImpl**:`arcana_infra::network::ProbeNetworkMonitor` —— 背景執行緒定期問可注入的 `Probe`;預設 `TcpProbe` 連 1.1.1.1:53 / 8.8.8.8:53,30 秒一次。
- **字型**:egui 內建字型沒有中日文;啟動時從系統字型清單挑第一個存在的(PingFang / 微軟正黑 / Noto CJK…)當 fallback,不內嵌。
- **UI 字串**:核心字串檔不動(M3 測試鎖了筆數);C# CoreMenu 外掛自帶的 locale 檔與本次新增字串以 plugin resources 註冊,查詢一律走 `get_from_any_plugin`。
- **測試**:C# 沒有 App/ViewModel 測試可移植。每頁 VM 單元測試 + NavGraph/Command/Theme/格式化測試 + shell 邏輯測試(真服務 + in-memory DB)+ `egui_kittest` 主流程(訂單清單→分頁開明細→編輯→存檔、新增訂單驗證訊息、語系切換、主題/強調色、外掛清單 + 選單)。kittest 不用 renderer,CI 無需額外系統套件。
- **截圖**:`cargo run -p arcana-app --features screenshots --bin screenshots -- docs/screen-rust`(wgpu 離屏繪製真 shell),輸出在 `docs/screen-rust/`,對照 C# `docs/screen/`。

## 8. 里程碑 5 紀錄(外掛 + FlowChart)
- **測試**:C# `Arcana.Plugins.Tests` 452 個 → 移植 429、未移植 23(皆無 Rust 對應:注入式 `ILogger` ×9、DI/service locator `GetService`/`ConfigureServices` ×5、`CancellationToken` ×2、反射 `Activator` ×1、`Exception.InnerException` ×1、`AssemblyLoadContext` 的 Collectible/Name/Unloading/GetUnloadStatus/Dispose 兩次 ×6 → 改以 host load/unload 與 WASM 沙箱測試對應)。FlowChart 27/27。另新增:WASM 沙箱 21(含負向:未授權讀檔/網路、fuel、記憶體上限、牆鐘逾時、ABI 版本、失敗不改狀態)、host 生命週期 12 + WASM 端到端 3、權限核准/版本庫/健康 15、FlowChart VM/serializer/kittest 28、UI kittest 2。
- **外掛 API**:同步(由 egui 每幀驅動),C# `Task` → `AppResult`;`IDisposable` → `Subscription`(不在 drop 時自動取消,與 C# 忘記 Dispose 同義);`PluginBase` → `PluginBehavior` + `BasePlugin`;`plugin is IPluginHealthCheck` → `Plugin::as_*` 方法;`WindowService` 回覆改經 command(不能 await)。manifest 欄位同 C# `plugin.json`,另加 `permissions`、`abiVersion`;`contributes.configuration` 兼作參數 schema → `schema::settings_form` 產生設定表單。
- **Host**:`PluginHost` 為 instance(無 global static,僅 inventory 的編譯期登錄表);內建 5 外掛 + FlowChart 以 `submit_builtin!`/直接註冊;第三方 = 目錄套件(`plugin.json` + `*.wasm`),C# 收 zip(未移植)。安裝 = lazy(manifest 貢獻佔位 + activation events);upgrade/rollback 有備份與交易式還原,版本歷史與健康快照存 SQLite(`PluginVersions`/`PluginMetadata`/`PluginHealthSnapshots`,在主資料庫,C# 是另一個 DbContext)。
- **WASM 沙箱**:wasmtime component model,WIT `arcana:plugin@1.0.0`;import 一律定義但未授權回 permission denied(log 丟棄);manifest 權限需經 PluginManager「核准」才變成 grant(`ReadLocalData`/`ReadFileSystem` → 只能讀外掛自己的目錄;`NetworkAccess` 目前沒有 host HTTP client,核准後仍拒絕);fuel/記憶體/牆鐘每次呼叫;每次呼叫新 Store,狀態僅成功時提交。
- **FlowChart**:見 crate 文件;偏離:undo 存 Diagram 複本、拖曳一次一筆 undo、存讀檔交給宿主、尺寸控點/轉折點拖曳未做、凹形以近似填色。FlowChart 註冊為編譯期內建(C# 是 lazily loaded 外部 DLL)。

### UI 部分
- **組裝**:`arcana_infra::plugin_runtime::PluginRuntime::start` 建 `HostServices`(H1 各 `*Impl`、`QueuedNavGraph`(已知路由 = C# `NavGraph.Routes` + `FlowChartEditorPage`)、`QueuedWindowService`、既有 `LocalizationServiceImpl`)與 `PluginHost`(`HostPaths::under(資料目錄)`、`SqlitePluginVersionStore` 存主資料庫),掛 `WasmLoader`,`register_compiled_builtins()`(inventory 的 5 個內建)+ UI 傳入的 `FlowChartPlugin`,`discover()`、`activate_all()`。放在 `UiServices::plugins`;`UiServices::from_arcana` 多一個 `plugins_root` 參數。
- **選單**:選單列改由 `MenuRegistry` 繪製(`MainMenu` 且無 parent 的為根,依 `parent_id` 建樹、依 `order` 排、`is_separator` 畫分隔線),點擊走 `CommandService::execute`。chrome.rs 寫死的選單已移除;快捷鍵與命令搜尋仍走 `CommandRegistry`。
- **外掛 → shell**:每幀 `pump_plugins()` 排空 `QueuedNavGraph`(轉 `NavRequest`;`FlowChartEditorPage` + `EditorArgs` → `Route::FlowChart { mode }`)、MessageBus 上的 `ShellCommand`(→ `CommandRegistry::by_name` → `Shell::execute`)、`QueuedWindowService`(→ toast)。C# 未實作頁面(`ProductCategoriesPage`)顯示「開發中」。
- **語系**:選單標題在註冊時以 `L()` 決定(同 C#);切換語系時 shell 對每個 Active 外掛 `reload`,重新註冊貢獻(C# 只有 CoreMenu 訂閱 `CultureChanged` 重建選單)。CoreMenu locale 改由外掛自己註冊,`arcana-ui/resources/menu` 移除。
- **PluginManager**:列 `host.all_plugins()`(含健康、錯誤數、已授予/待核准權限);安裝(套件目錄路徑)、啟用、停用、重載、升級、回滾(選版本,`available_versions`)、解除安裝、核准權限、健康檢查皆為 async task,結果 toast 後重新載入清單。manifest 有 `contributes.configuration` 時以 `settings_form` 產生設定表單,值存 `plugin.<id>.<key>`。外掛有 `ui_tree()`(WASM 宣告式 UI)時由 host 繪製,button 執行 `commands.execute(command)`。
- **FlowChart 分頁**:`Route::FlowChart` 開文件分頁(每次新分頁),`Page::show` 改 `&mut self` 讓畫布直接改 VM;開檔 / 另存新檔以路徑輸入對話框代替原生檔案對話框,讀寫在 `spawn_blocking`。
- **分層表**:`arcana-ui` 可依賴 `arcana-plugin-api`、`arcana-plugin-host`、`arcana-plugin-flowchart`(繪製外掛貢獻 + FlowChart 文件分頁)。
- **kittest**:PluginManager 停用→啟用(狀態 Active↔Deactivated、`menu.business.order` 消失/出現、toast)、Tools → FlowChart → New FlowChart 開分頁 + Add Node + Undo;既有「Business → Product → Product Management」改走巢狀外掛選單。
- **截圖對照**(`docs/screen-rust/flowchart.png` vs C# `docs/screen/Flow Chart Plugin.png`):同樣有工具列、格線畫布、屬性面板、狀態列;差異 —— C# 左側有「Shapes」形狀清單面板(這裡是工具列下拉選單)、側欄有 FunctionTree 的「FlowChart」項(這裡側欄仍是固定清單,沒讀 `FunctionTree` 貢獻)、屬性面板 C# 有 Diagram Name / Description / Show Grid / Snap / Grid Size(這裡未選取時只顯示「Nothing selected」)、工具列 C# 有圖示 + 文字。
