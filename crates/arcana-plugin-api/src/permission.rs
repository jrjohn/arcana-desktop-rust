//! `PluginPermission` flags and the permission request / manifest records
//! (C# `IPluginPermissions.cs`).

use std::collections::BTreeMap;
use std::fmt;
use std::ops::{BitAnd, BitOr, BitOrAssign, Not};

use serde::{Deserialize, Serialize};

/// C# `[Flags] enum PluginPermission`. Same bit positions.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PluginPermission(pub u32);

macro_rules! perms {
    ($($name:ident = $bit:expr, $text:literal;)*) => {
        impl PluginPermission {
            $(pub const $name: Self = Self(1 << $bit);)*
            /// Every single-bit permission with its C# name.
            pub const NAMED: &'static [(Self, &'static str)] = &[$((Self::$name, $text)),*];
        }
    };
}

perms! {
    READ_LOCAL_DATA = 0, "ReadLocalData";
    WRITE_LOCAL_DATA = 1, "WriteLocalData";
    READ_FILE_SYSTEM = 2, "ReadFileSystem";
    WRITE_FILE_SYSTEM = 3, "WriteFileSystem";
    NETWORK_ACCESS = 4, "NetworkAccess";
    WEB_SOCKET_ACCESS = 5, "WebSocketAccess";
    READ_DATABASE = 6, "ReadDatabase";
    WRITE_DATABASE = 7, "WriteDatabase";
    CREATE_WINDOWS = 8, "CreateWindows";
    CREATE_DIALOGS = 9, "CreateDialogs";
    ACCESS_CLIPBOARD = 10, "AccessClipboard";
    SHOW_NOTIFICATIONS = 11, "ShowNotifications";
    INTER_PLUGIN_COMM = 12, "InterPluginComm";
    ACCESS_SHARED_STATE = 13, "AccessSharedState";
    EXECUTE_COMMANDS = 14, "ExecuteCommands";
    REGISTER_MENUS = 15, "RegisterMenus";
    REGISTER_VIEWS = 16, "RegisterViews";
    ACCESS_CREDENTIALS = 17, "AccessCredentials";
    EXECUTE_PROCESS = 18, "ExecuteProcess";
    ACCESS_HARDWARE = 19, "AccessHardware";
}

impl PluginPermission {
    pub const NONE: Self = Self(0);
    pub const BASIC_PLUGIN: Self = Self(
        Self::READ_LOCAL_DATA.0
            | Self::WRITE_LOCAL_DATA.0
            | Self::CREATE_DIALOGS.0
            | Self::INTER_PLUGIN_COMM.0
            | Self::REGISTER_MENUS.0
            | Self::REGISTER_VIEWS.0,
    );
    pub const NETWORK_PLUGIN: Self = Self(Self::BASIC_PLUGIN.0 | Self::NETWORK_ACCESS.0);
    pub const FULL_ACCESS: Self = Self((1 << 20) - 1);

    /// C# `HasFlag`: every bit of `other` is set.
    #[must_use]
    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
    /// Single-bit permissions set in `self`.
    #[must_use]
    pub fn flags(self) -> Vec<Self> {
        Self::NAMED
            .iter()
            .filter(|(p, _)| self.contains(*p))
            .map(|(p, _)| *p)
            .collect()
    }
    /// Parse one C# name (`"NetworkAccess"`), the set names or a `|`/`,`-separated list.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let mut out = Self::NONE;
        for part in s.split(['|', ',']).map(str::trim).filter(|p| !p.is_empty()) {
            out |= match part {
                "None" => Self::NONE,
                "BasicPlugin" => Self::BASIC_PLUGIN,
                "NetworkPlugin" => Self::NETWORK_PLUGIN,
                "FullAccess" => Self::FULL_ACCESS,
                other => {
                    Self::NAMED
                        .iter()
                        .find(|(_, n)| n.eq_ignore_ascii_case(other))?
                        .0
                }
            };
        }
        Some(out)
    }
}

impl fmt::Debug for PluginPermission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// C# `Enum.ToString()` for flags: `"None"`, `"FullAccess"` or `"A, B"`.
impl fmt::Display for PluginPermission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            return f.write_str("None");
        }
        if *self == Self::FULL_ACCESS {
            return f.write_str("FullAccess");
        }
        let names: Vec<&str> = Self::NAMED
            .iter()
            .filter(|(p, _)| self.contains(*p))
            .map(|(_, n)| *n)
            .collect();
        f.write_str(&names.join(", "))
    }
}

impl BitOr for PluginPermission {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}
impl BitOrAssign for PluginPermission {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}
impl BitAnd for PluginPermission {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}
impl Not for PluginPermission {
    type Output = Self;
    fn not(self) -> Self {
        Self(!self.0 & Self::FULL_ACCESS.0)
    }
}

/// C# `PermissionRequest`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionRequest {
    pub plugin_id: String,
    pub permission: PluginPermission,
    pub resource: Option<String>,
    pub reason: Option<String>,
}

/// C# `PermissionCheckResult`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PermissionCheckResult {
    pub granted: bool,
    pub denied_permissions: PluginPermission,
    pub message: Option<String>,
}

impl PermissionCheckResult {
    #[must_use]
    pub fn allow() -> Self {
        Self {
            granted: true,
            ..Self::default()
        }
    }
    #[must_use]
    pub fn deny(denied: PluginPermission, message: Option<String>) -> Self {
        Self {
            granted: false,
            denied_permissions: denied,
            message,
        }
    }
}

/// C# `PluginPermissionManifest` (also the manifest's `permissions` block).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PluginPermissionManifest {
    pub required_permissions: PluginPermission,
    pub optional_permissions: PluginPermission,
    pub permission_reasons: BTreeMap<String, String>,
}
