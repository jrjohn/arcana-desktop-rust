//! Host side of the `arcana:plugin` imports, each gated by [`Grants`].

use std::collections::BTreeMap;
use std::path::{Component as PathComponent, Path};

use wasmtime::component::Linker;
use wasmtime::{StoreContextMut, StoreLimits};

use crate::Grants;

/// Largest file `host-fs.read-file` will return.
const MAX_READ: u64 = 1024 * 1024;
const PKG: &str = "arcana:plugin";
const VER: &str = "@1.0.0";

pub(crate) struct Host {
    grants: Grants,
    /// State as the call sees it; replaces the committed map only on success.
    pub(crate) pending: BTreeMap<String, String>,
    pub(crate) logs: Vec<String>,
    pub(crate) limits: StoreLimits,
}

impl Host {
    pub(crate) fn new(
        grants: Grants,
        pending: BTreeMap<String, String>,
        limits: StoreLimits,
    ) -> Self {
        Self {
            grants,
            pending,
            logs: Vec::new(),
            limits,
        }
    }
}

fn denied(what: &str) -> Result<String, String> {
    Err(format!("permission denied: {what} not granted"))
}

pub(crate) fn define(linker: &mut Linker<Host>) -> wasmtime::Result<()> {
    linker
        .instance(&format!("{PKG}/host-log{VER}"))?
        .func_wrap(
            "log",
            |mut cx: StoreContextMut<'_, Host>, (msg,): (String,)| {
                let h = cx.data_mut();
                if h.grants.log && h.logs.len() < 1000 {
                    h.logs.push(msg);
                }
                Ok(())
            },
        )?;
    linker.instance(&format!("{PKG}/host-fs{VER}"))?.func_wrap(
        "read-file",
        |cx: StoreContextMut<'_, Host>, (path,): (String,)| {
            let r = match &cx.data().grants.fs_read {
                Some(root) => read_below(root, &path),
                None => denied("file read"),
            };
            Ok((r,))
        },
    )?;
    linker
        .instance(&format!("{PKG}/host-net{VER}"))?
        .func_wrap(
            "http-get",
            |cx: StoreContextMut<'_, Host>, (url,): (String,)| {
                let r = match &cx.data().grants.net {
                    Some(client) => client.get(&url),
                    None => denied("network"),
                };
                Ok((r,))
            },
        )?;
    let mut state = linker.instance(&format!("{PKG}/host-state{VER}"))?;
    state.func_wrap("get", |cx: StoreContextMut<'_, Host>, (key,): (String,)| {
        Ok((cx.data().pending.get(&key).cloned(),))
    })?;
    state.func_wrap(
        "set",
        |mut cx: StoreContextMut<'_, Host>, (key, value): (String, String)| {
            cx.data_mut().pending.insert(key, value);
            Ok(())
        },
    )?;
    Ok(())
}

/// Read `rel` only if it is a plain relative path that stays below `root` after symlinks.
fn read_below(root: &Path, rel: &str) -> Result<String, String> {
    let p = Path::new(rel);
    if !p
        .components()
        .all(|c| matches!(c, PathComponent::Normal(_) | PathComponent::CurDir))
    {
        return Err(format!(
            "permission denied: `{rel}` leaves the granted root"
        ));
    }
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let full = root.join(p).canonicalize().map_err(|e| e.to_string())?;
    if !full.starts_with(&root) {
        return Err(format!(
            "permission denied: `{rel}` leaves the granted root"
        ));
    }
    let len = std::fs::metadata(&full).map_err(|e| e.to_string())?.len();
    if len > MAX_READ {
        return Err(format!("file larger than {MAX_READ} bytes"));
    }
    std::fs::read_to_string(&full).map_err(|e| e.to_string())
}
