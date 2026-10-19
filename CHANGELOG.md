# Changelog

## [0.2.1](https://github.com/jrjohn/arcana-desktop-rust/compare/v0.2.0...v0.2.1) (2026-10-19)


### Bug Fixes

* **ci:** Windows 不再每次 cargo clean,改關 incremental ([#22](https://github.com/jrjohn/arcana-desktop-rust/issues/22)) ([ea31a7d](https://github.com/jrjohn/arcana-desktop-rust/commit/ea31a7d891f2dd1ac15c26e3e6cc97afb5411d88))
* **ci:** Windows 所有分支共用一個 CARGO_TARGET_DIR ([#23](https://github.com/jrjohn/arcana-desktop-rust/issues/23)) ([6ca70b6](https://github.com/jrjohn/arcana-desktop-rust/commit/6ca70b6c5f1afc2a250b1cd85ee29236191bb11c))
* **ci:** 不再漏容器塞爆 /data;Windows 固定 Rust 版本 ([#20](https://github.com/jrjohn/arcana-desktop-rust/issues/20)) ([6e16d9f](https://github.com/jrjohn/arcana-desktop-rust/commit/6e16d9fbf0ff907dc07f44a3779c3daabd20c92b))
* **deps:** update rust crate base64 to 0.23 ([#18](https://github.com/jrjohn/arcana-desktop-rust/issues/18)) ([1338dec](https://github.com/jrjohn/arcana-desktop-rust/commit/1338dec1b03d08ff706481309e73d1e96914651a))
* **deps:** update rust crate dirs to v7 ([#17](https://github.com/jrjohn/arcana-desktop-rust/issues/17)) ([48405a9](https://github.com/jrjohn/arcana-desktop-rust/commit/48405a9dc571642f1aac825f926695f6d161ec79))
* **deps:** update rust docker tag to v1.99 ([#24](https://github.com/jrjohn/arcana-desktop-rust/issues/24)) ([0c33564](https://github.com/jrjohn/arcana-desktop-rust/commit/0c3356402db4b21c163cd18c01536926fc75ad6c))

## [0.2.0](https://github.com/jrjohn/arcana-desktop-rust/compare/v0.1.0...v0.2.0) (2026-10-16)


### Features

* milestone 1 — workspace 骨架、分層檢查、CI 閘 ([#1](https://github.com/jrjohn/arcana-desktop-rust/issues/1)) ([52ed15c](https://github.com/jrjohn/arcana-desktop-rust/commit/52ed15c0df65032e44d93715c02a9dbfe8c295b1))
* milestone 2 — arcana-core / arcana-domain / arcana-data ([#2](https://github.com/jrjohn/arcana-desktop-rust/issues/2)) ([435076d](https://github.com/jrjohn/arcana-desktop-rust/commit/435076d40ce856235582a14c6e866fe55622a524))
* milestone 3 — arcana-sync / arcana-infra ([#3](https://github.com/jrjohn/arcana-desktop-rust/issues/3)) ([72a7e06](https://github.com/jrjohn/arcana-desktop-rust/commit/72a7e06a5db9c1746afec916019b752184cfa510))
* milestone 4 — egui UI (shell, dock, NavGraph, pages, kittest) ([#4](https://github.com/jrjohn/arcana-desktop-rust/issues/4)) ([ae5da03](https://github.com/jrjohn/arcana-desktop-rust/commit/ae5da039d99dc472f51f74d4bdb4a7fca2f3f97b))
* milestone 5 — plugin system (host + WASM sandbox) + FlowChart ([#6](https://github.com/jrjohn/arcana-desktop-rust/issues/6)) ([a6d25da](https://github.com/jrjohn/arcana-desktop-rust/commit/a6d25da48544f3d4bca35681a8287c8fc738382f))
* milestone 6 — cross-platform (Windows/macOS/Linux) + M5 parity gaps ([#7](https://github.com/jrjohn/arcana-desktop-rust/issues/7)) ([2c4c014](https://github.com/jrjohn/arcana-desktop-rust/commit/2c4c01462537436767deb4e5d4ed5226720a3443))


### Bug Fixes

* **ci:** clean Rust target/ before Windows build to clear corrupt incremental cache ([1dba14b](https://github.com/jrjohn/arcana-desktop-rust/commit/1dba14b13f873fa122e9cd2080a9a9ee944ac7f1))
* **package:** .msi 打包缺 License.rtf ([#8](https://github.com/jrjohn/arcana-desktop-rust/issues/8)) ([99b7691](https://github.com/jrjohn/arcana-desktop-rust/commit/99b7691917b7c9f602f10e0a093da5ac81d32cb2))
