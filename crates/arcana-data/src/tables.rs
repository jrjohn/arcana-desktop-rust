//! Table mappings for every entity in `AppDbContext`, plus the `SyncQueue` row type
//! (C# `SyncQueueItem`, `SyncOperation`, `SyncStatus`, defined in `AppDbContext.cs`).

use arcana_core::{AppResult, AuditInfo, SoftDeleteInfo, SyncInfo, Timestamp, int_enum, utc_now};
use arcana_domain::{
    AppPermission, AuditEventType, AuditLog, BaseEntity, Customer, Order, OrderItem, OrderStatus,
    PaymentMethod, PaymentStatus, Product, ProductCategory, Role, RolePermission, User,
    UserPermission, UserRole,
};
use sqlx::sqlite::SqliteRow;

use crate::row::{FromSql, SqlValue, TableEntity, ToSql, sql_int_enum};

sql_int_enum!(OrderStatus, PaymentStatus, PaymentMethod, AuditEventType);

fn base_values(b: &BaseEntity) -> [SqlValue; 10] {
    [
        b.audit.created_at.to_sql(),
        b.audit.created_by.to_sql(),
        b.audit.modified_at.to_sql(),
        b.audit.modified_by.to_sql(),
        b.soft_delete.is_deleted.to_sql(),
        b.soft_delete.deleted_at.to_sql(),
        b.soft_delete.deleted_by.to_sql(),
        b.sync.sync_id.to_sql(),
        b.sync.last_sync_at.to_sql(),
        b.sync.is_pending_sync.to_sql(),
    ]
}

fn base_from_row(row: &SqliteRow) -> AppResult<BaseEntity> {
    Ok(BaseEntity {
        id: i32::from_sql(row, "Id")?,
        audit: AuditInfo {
            created_at: FromSql::from_sql(row, "CreatedAt")?,
            created_by: FromSql::from_sql(row, "CreatedBy")?,
            modified_at: FromSql::from_sql(row, "ModifiedAt")?,
            modified_by: FromSql::from_sql(row, "ModifiedBy")?,
        },
        soft_delete: SoftDeleteInfo {
            is_deleted: FromSql::from_sql(row, "IsDeleted")?,
            deleted_at: FromSql::from_sql(row, "DeletedAt")?,
            deleted_by: FromSql::from_sql(row, "DeletedBy")?,
        },
        sync: SyncInfo {
            sync_id: FromSql::from_sql(row, "SyncId")?,
            last_sync_at: FromSql::from_sql(row, "LastSyncAt")?,
            is_pending_sync: FromSql::from_sql(row, "IsPendingSync")?,
        },
    })
}

/// Map a `BaseEntity`-derived domain struct: its own columns, then the 10 base columns.
/// Navigation fields are left at their defaults (empty) when reading.
macro_rules! base_table {
    ($t:ty, $table:literal, filter = $filter:literal, { $($col:literal => $field:ident),+ $(,)? }) => {
        impl TableEntity for $t {
            const TABLE: &'static str = $table;
            const COLUMNS: &'static [&'static str] = &[
                $($col,)+
                "CreatedAt", "CreatedBy", "ModifiedAt", "ModifiedBy", "IsDeleted", "DeletedAt",
                "DeletedBy", "SyncId", "LastSyncAt", "IsPendingSync",
            ];
            const QUERY_FILTER: bool = $filter;

            fn id(&self) -> i32 {
                self.base.id
            }
            fn set_id(&mut self, id: i32) {
                self.base.id = id;
            }
            fn values(&self) -> Vec<SqlValue> {
                let mut v = vec![$(self.$field.to_sql()),+];
                v.extend(base_values(&self.base));
                v
            }
            #[allow(
                clippy::needless_update,
                reason = "entities without navigation fields have nothing left to default"
            )]
            fn from_row(row: &SqliteRow) -> AppResult<Self> {
                Ok(Self {
                    base: base_from_row(row)?,
                    $($field: FromSql::from_sql(row, $col)?,)+
                    ..Self::default()
                })
            }
            fn audit_mut(&mut self) -> Option<&mut AuditInfo> {
                Some(&mut self.base.audit)
            }
            fn soft_delete_mut(&mut self) -> Option<&mut SoftDeleteInfo> {
                Some(&mut self.base.soft_delete)
            }
            fn sync_mut(&mut self) -> Option<&mut SyncInfo> {
                Some(&mut self.base.sync)
            }
        }
    };
}

base_table!(Customer, "Customers", filter = true, {
    "Code" => code, "Name" => name, "ContactName" => contact_name, "Phone" => phone,
    "Email" => email, "Address" => address, "City" => city, "State" => state,
    "PostalCode" => postal_code, "Country" => country, "TaxId" => tax_id,
    "CreditLimit" => credit_limit, "Balance" => balance, "IsActive" => is_active,
    "Notes" => notes,
});

base_table!(ProductCategory, "ProductCategories", filter = true, {
    "Code" => code, "Name" => name, "ParentId" => parent_id, "SortOrder" => sort_order,
});

base_table!(Product, "Products", filter = true, {
    "Code" => code, "Name" => name, "Description" => description, "CategoryId" => category_id,
    "Unit" => unit, "Price" => price, "Cost" => cost, "StockQuantity" => stock_quantity,
    "MinStockLevel" => min_stock_level, "MaxStockLevel" => max_stock_level,
    "IsActive" => is_active, "Barcode" => barcode, "Weight" => weight,
});

base_table!(Order, "Orders", filter = true, {
    "OrderNumber" => order_number, "OrderDate" => order_date, "CustomerId" => customer_id,
    "CustomerName" => customer_name, "Status" => status, "PaymentStatus" => payment_status,
    "PaymentMethod" => payment_method, "Subtotal" => subtotal, "TaxRate" => tax_rate,
    "TaxAmount" => tax_amount, "DiscountAmount" => discount_amount,
    "ShippingCost" => shipping_cost, "TotalAmount" => total_amount,
    "PaidAmount" => paid_amount, "ShippingAddress" => shipping_address,
    "ShippingCity" => shipping_city, "ShippingPostalCode" => shipping_postal_code,
    "ExpectedDeliveryDate" => expected_delivery_date,
    "ActualDeliveryDate" => actual_delivery_date, "Notes" => notes,
});

base_table!(OrderItem, "OrderItems", filter = true, {
    "OrderId" => order_id, "LineNumber" => line_number, "ProductId" => product_id,
    "ProductCode" => product_code, "ProductName" => product_name, "Unit" => unit,
    "Quantity" => quantity, "UnitPrice" => unit_price, "DiscountPercent" => discount_percent,
    "Notes" => notes,
});

base_table!(User, "Users", filter = true, {
    "Username" => username, "Email" => email, "DisplayName" => display_name,
    "PasswordHash" => password_hash, "PasswordSalt" => password_salt,
    "IsActive" => is_active, "IsLocked" => is_locked, "LockedUntil" => locked_until,
    "FailedLoginAttempts" => failed_login_attempts, "LastLoginAt" => last_login_at,
    "PasswordChangedAt" => password_changed_at, "MustChangePassword" => must_change_password,
    "RefreshToken" => refresh_token, "RefreshTokenExpiresAt" => refresh_token_expires_at,
});

base_table!(Role, "Roles", filter = true, {
    "Name" => name, "DisplayName" => display_name, "Description" => description,
    "IsSystem" => is_system, "Priority" => priority,
});

base_table!(UserRole, "UserRoles", filter = true, {
    "UserId" => user_id, "RoleId" => role_id, "AssignedAt" => assigned_at,
    "AssignedBy" => assigned_by, "ExpiresAt" => expires_at,
});

base_table!(AppPermission, "Permissions", filter = true, {
    "Code" => code, "DisplayName" => display_name, "Description" => description,
    "Category" => category, "IsSystem" => is_system,
});

base_table!(RolePermission, "RolePermissions", filter = true, {
    "RoleId" => role_id, "PermissionId" => permission_id, "GrantedAt" => granted_at,
    "GrantedBy" => granted_by,
});

base_table!(UserPermission, "UserPermissions", filter = true, {
    "UserId" => user_id, "PermissionId" => permission_id, "IsGranted" => is_granted,
    "AssignedAt" => assigned_at, "AssignedBy" => assigned_by, "ExpiresAt" => expires_at,
});

// C# configures no `HasQueryFilter` for AuditLog, so deleted rows stay visible.
base_table!(AuditLog, "AuditLogs", filter = false, {
    "EventType" => event_type, "UserId" => user_id, "Username" => username,
    "ClientInfo" => client_info, "Resource" => resource, "Action" => action,
    "Details" => details, "IsSuccess" => is_success, "ErrorMessage" => error_message,
    "Timestamp" => timestamp,
});

int_enum! {
    /// C# `SyncOperation`.
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
    pub enum SyncOperation {
        #[default]
        Create = 0,
        Update = 1,
        Delete = 2,
    }
}

int_enum! {
    /// C# `SyncStatus`.
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
    pub enum SyncStatus {
        #[default]
        Pending = 0,
        InProgress = 1,
        Completed = 2,
        Failed = 3,
    }
}

sql_int_enum!(SyncOperation, SyncStatus);

/// One queued offline change (C# `SyncQueueItem`). Not a `BaseEntity`: no audit, no soft
/// delete, no sync bookkeeping — deleting it removes the row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncQueueItem {
    pub id: i32,
    pub entity_type: String,
    pub entity_id: String,
    pub operation: SyncOperation,
    pub payload: Option<String>,
    pub created_at: Timestamp,
    pub retry_count: i32,
    pub last_error: Option<String>,
    pub status: SyncStatus,
}

impl Default for SyncQueueItem {
    fn default() -> Self {
        Self {
            id: 0,
            entity_type: String::new(),
            entity_id: String::new(),
            operation: SyncOperation::default(),
            payload: None,
            created_at: utc_now(),
            retry_count: 0,
            last_error: None,
            status: SyncStatus::default(),
        }
    }
}

impl TableEntity for SyncQueueItem {
    const TABLE: &'static str = "SyncQueue";
    const COLUMNS: &'static [&'static str] = &[
        "EntityType",
        "EntityId",
        "Operation",
        "Payload",
        "CreatedAt",
        "RetryCount",
        "LastError",
        "Status",
    ];
    const QUERY_FILTER: bool = false;

    fn id(&self) -> i32 {
        self.id
    }
    fn set_id(&mut self, id: i32) {
        self.id = id;
    }
    fn values(&self) -> Vec<SqlValue> {
        vec![
            self.entity_type.to_sql(),
            self.entity_id.to_sql(),
            self.operation.to_sql(),
            self.payload.to_sql(),
            self.created_at.to_sql(),
            self.retry_count.to_sql(),
            self.last_error.to_sql(),
            self.status.to_sql(),
        ]
    }
    fn from_row(row: &SqliteRow) -> AppResult<Self> {
        Ok(Self {
            id: FromSql::from_sql(row, "Id")?,
            entity_type: FromSql::from_sql(row, "EntityType")?,
            entity_id: FromSql::from_sql(row, "EntityId")?,
            operation: FromSql::from_sql(row, "Operation")?,
            payload: FromSql::from_sql(row, "Payload")?,
            created_at: FromSql::from_sql(row, "CreatedAt")?,
            retry_count: FromSql::from_sql(row, "RetryCount")?,
            last_error: FromSql::from_sql(row, "LastError")?,
            status: FromSql::from_sql(row, "Status")?,
        })
    }
}
