-- Initial schema: the 13 tables of C# `AppDbContext` (Arcana.Data/Local/AppDbContext.cs).
-- Types follow EF Core's SQLite provider: decimal/DateTime/Guid -> TEXT, bool/enum -> INTEGER.
-- EF `HasMaxLength` is not enforced by SQLite DDL; lengths are enforced by the validators.
-- Index and constraint names follow EF Core conventions (IX_/FK_/PK_), including the indexes
-- EF adds automatically for foreign keys.

CREATE TABLE "ProductCategories" (
    "Id" INTEGER NOT NULL CONSTRAINT "PK_ProductCategories" PRIMARY KEY AUTOINCREMENT,
    "Code" TEXT NOT NULL,
    "Name" TEXT NOT NULL,
    "ParentId" INTEGER NULL,
    "SortOrder" INTEGER NOT NULL,
    "CreatedAt" TEXT NOT NULL,
    "CreatedBy" TEXT NULL,
    "ModifiedAt" TEXT NULL,
    "ModifiedBy" TEXT NULL,
    "IsDeleted" INTEGER NOT NULL,
    "DeletedAt" TEXT NULL,
    "DeletedBy" TEXT NULL,
    "SyncId" TEXT NOT NULL,
    "LastSyncAt" TEXT NULL,
    "IsPendingSync" INTEGER NOT NULL,
    CONSTRAINT "FK_ProductCategories_ProductCategories_ParentId" FOREIGN KEY ("ParentId") REFERENCES "ProductCategories" ("Id") ON DELETE RESTRICT
);

CREATE UNIQUE INDEX "IX_ProductCategories_Code" ON "ProductCategories" ("Code");

CREATE INDEX "IX_ProductCategories_ParentId" ON "ProductCategories" ("ParentId");

CREATE TABLE "Customers" (
    "Id" INTEGER NOT NULL CONSTRAINT "PK_Customers" PRIMARY KEY AUTOINCREMENT,
    "Code" TEXT NOT NULL,
    "Name" TEXT NOT NULL,
    "ContactName" TEXT NULL,
    "Phone" TEXT NULL,
    "Email" TEXT NULL,
    "Address" TEXT NULL,
    "City" TEXT NULL,
    "State" TEXT NULL,
    "PostalCode" TEXT NULL,
    "Country" TEXT NULL,
    "TaxId" TEXT NULL,
    "CreditLimit" TEXT NOT NULL,
    "Balance" TEXT NOT NULL,
    "IsActive" INTEGER NOT NULL,
    "Notes" TEXT NULL,
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

CREATE UNIQUE INDEX "IX_Customers_Code" ON "Customers" ("Code");

CREATE INDEX "IX_Customers_SyncId" ON "Customers" ("SyncId");

CREATE TABLE "Products" (
    "Id" INTEGER NOT NULL CONSTRAINT "PK_Products" PRIMARY KEY AUTOINCREMENT,
    "Code" TEXT NOT NULL,
    "Name" TEXT NOT NULL,
    "Description" TEXT NULL,
    "CategoryId" INTEGER NULL,
    "Unit" TEXT NOT NULL,
    "Price" TEXT NOT NULL,
    "Cost" TEXT NOT NULL,
    "StockQuantity" TEXT NOT NULL,
    "MinStockLevel" TEXT NOT NULL,
    "MaxStockLevel" TEXT NOT NULL,
    "IsActive" INTEGER NOT NULL,
    "Barcode" TEXT NULL,
    "Weight" TEXT NULL,
    "CreatedAt" TEXT NOT NULL,
    "CreatedBy" TEXT NULL,
    "ModifiedAt" TEXT NULL,
    "ModifiedBy" TEXT NULL,
    "IsDeleted" INTEGER NOT NULL,
    "DeletedAt" TEXT NULL,
    "DeletedBy" TEXT NULL,
    "SyncId" TEXT NOT NULL,
    "LastSyncAt" TEXT NULL,
    "IsPendingSync" INTEGER NOT NULL,
    CONSTRAINT "FK_Products_ProductCategories_CategoryId" FOREIGN KEY ("CategoryId") REFERENCES "ProductCategories" ("Id") ON DELETE SET NULL
);

CREATE UNIQUE INDEX "IX_Products_Code" ON "Products" ("Code");

CREATE INDEX "IX_Products_Barcode" ON "Products" ("Barcode");

CREATE INDEX "IX_Products_SyncId" ON "Products" ("SyncId");

CREATE INDEX "IX_Products_CategoryId" ON "Products" ("CategoryId");

CREATE TABLE "Orders" (
    "Id" INTEGER NOT NULL CONSTRAINT "PK_Orders" PRIMARY KEY AUTOINCREMENT,
    "OrderNumber" TEXT NOT NULL,
    "OrderDate" TEXT NOT NULL,
    "CustomerId" INTEGER NOT NULL,
    "CustomerName" TEXT NOT NULL,
    "Status" INTEGER NOT NULL,
    "PaymentStatus" INTEGER NOT NULL,
    "PaymentMethod" INTEGER NOT NULL,
    "Subtotal" TEXT NOT NULL,
    "TaxRate" TEXT NOT NULL,
    "TaxAmount" TEXT NOT NULL,
    "DiscountAmount" TEXT NOT NULL,
    "ShippingCost" TEXT NOT NULL,
    "TotalAmount" TEXT NOT NULL,
    "PaidAmount" TEXT NOT NULL,
    "ShippingAddress" TEXT NULL,
    "ShippingCity" TEXT NULL,
    "ShippingPostalCode" TEXT NULL,
    "ExpectedDeliveryDate" TEXT NULL,
    "ActualDeliveryDate" TEXT NULL,
    "Notes" TEXT NULL,
    "CreatedAt" TEXT NOT NULL,
    "CreatedBy" TEXT NULL,
    "ModifiedAt" TEXT NULL,
    "ModifiedBy" TEXT NULL,
    "IsDeleted" INTEGER NOT NULL,
    "DeletedAt" TEXT NULL,
    "DeletedBy" TEXT NULL,
    "SyncId" TEXT NOT NULL,
    "LastSyncAt" TEXT NULL,
    "IsPendingSync" INTEGER NOT NULL,
    CONSTRAINT "FK_Orders_Customers_CustomerId" FOREIGN KEY ("CustomerId") REFERENCES "Customers" ("Id") ON DELETE RESTRICT
);

CREATE UNIQUE INDEX "IX_Orders_OrderNumber" ON "Orders" ("OrderNumber");

CREATE INDEX "IX_Orders_OrderDate" ON "Orders" ("OrderDate");

CREATE INDEX "IX_Orders_CustomerId" ON "Orders" ("CustomerId");

CREATE INDEX "IX_Orders_SyncId" ON "Orders" ("SyncId");

CREATE TABLE "OrderItems" (
    "Id" INTEGER NOT NULL CONSTRAINT "PK_OrderItems" PRIMARY KEY AUTOINCREMENT,
    "OrderId" INTEGER NOT NULL,
    "LineNumber" INTEGER NOT NULL,
    "ProductId" INTEGER NOT NULL,
    "ProductCode" TEXT NOT NULL,
    "ProductName" TEXT NOT NULL,
    "Unit" TEXT NOT NULL,
    "Quantity" TEXT NOT NULL,
    "UnitPrice" TEXT NOT NULL,
    "DiscountPercent" TEXT NOT NULL,
    "Notes" TEXT NULL,
    "CreatedAt" TEXT NOT NULL,
    "CreatedBy" TEXT NULL,
    "ModifiedAt" TEXT NULL,
    "ModifiedBy" TEXT NULL,
    "IsDeleted" INTEGER NOT NULL,
    "DeletedAt" TEXT NULL,
    "DeletedBy" TEXT NULL,
    "SyncId" TEXT NOT NULL,
    "LastSyncAt" TEXT NULL,
    "IsPendingSync" INTEGER NOT NULL,
    CONSTRAINT "FK_OrderItems_Orders_OrderId" FOREIGN KEY ("OrderId") REFERENCES "Orders" ("Id") ON DELETE CASCADE,
    CONSTRAINT "FK_OrderItems_Products_ProductId" FOREIGN KEY ("ProductId") REFERENCES "Products" ("Id") ON DELETE RESTRICT
);

CREATE UNIQUE INDEX "IX_OrderItems_OrderId_LineNumber" ON "OrderItems" ("OrderId", "LineNumber");

CREATE INDEX "IX_OrderItems_ProductId" ON "OrderItems" ("ProductId");

CREATE TABLE "SyncQueue" (
    "Id" INTEGER NOT NULL CONSTRAINT "PK_SyncQueue" PRIMARY KEY AUTOINCREMENT,
    "EntityType" TEXT NOT NULL,
    "EntityId" TEXT NOT NULL,
    "Operation" INTEGER NOT NULL,
    "Payload" TEXT NULL,
    "CreatedAt" TEXT NOT NULL,
    "RetryCount" INTEGER NOT NULL,
    "LastError" TEXT NULL,
    "Status" INTEGER NOT NULL
);

CREATE INDEX "IX_SyncQueue_EntityType" ON "SyncQueue" ("EntityType");

CREATE INDEX "IX_SyncQueue_CreatedAt" ON "SyncQueue" ("CreatedAt");

CREATE INDEX "IX_SyncQueue_Status" ON "SyncQueue" ("Status");

CREATE TABLE "Users" (
    "Id" INTEGER NOT NULL CONSTRAINT "PK_Users" PRIMARY KEY AUTOINCREMENT,
    "Username" TEXT NOT NULL,
    "Email" TEXT NULL,
    "DisplayName" TEXT NOT NULL,
    "PasswordHash" TEXT NOT NULL,
    "PasswordSalt" TEXT NULL,
    "IsActive" INTEGER NOT NULL,
    "IsLocked" INTEGER NOT NULL,
    "LockedUntil" TEXT NULL,
    "FailedLoginAttempts" INTEGER NOT NULL,
    "LastLoginAt" TEXT NULL,
    "PasswordChangedAt" TEXT NULL,
    "MustChangePassword" INTEGER NOT NULL,
    "RefreshToken" TEXT NULL,
    "RefreshTokenExpiresAt" TEXT NULL,
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

CREATE UNIQUE INDEX "IX_Users_Username" ON "Users" ("Username");

CREATE INDEX "IX_Users_Email" ON "Users" ("Email");

CREATE INDEX "IX_Users_SyncId" ON "Users" ("SyncId");

CREATE TABLE "Roles" (
    "Id" INTEGER NOT NULL CONSTRAINT "PK_Roles" PRIMARY KEY AUTOINCREMENT,
    "Name" TEXT NOT NULL,
    "DisplayName" TEXT NOT NULL,
    "Description" TEXT NULL,
    "IsSystem" INTEGER NOT NULL,
    "Priority" INTEGER NOT NULL,
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

CREATE UNIQUE INDEX "IX_Roles_Name" ON "Roles" ("Name");

CREATE TABLE "UserRoles" (
    "Id" INTEGER NOT NULL CONSTRAINT "PK_UserRoles" PRIMARY KEY AUTOINCREMENT,
    "UserId" INTEGER NOT NULL,
    "RoleId" INTEGER NOT NULL,
    "AssignedAt" TEXT NOT NULL,
    "AssignedBy" TEXT NULL,
    "ExpiresAt" TEXT NULL,
    "CreatedAt" TEXT NOT NULL,
    "CreatedBy" TEXT NULL,
    "ModifiedAt" TEXT NULL,
    "ModifiedBy" TEXT NULL,
    "IsDeleted" INTEGER NOT NULL,
    "DeletedAt" TEXT NULL,
    "DeletedBy" TEXT NULL,
    "SyncId" TEXT NOT NULL,
    "LastSyncAt" TEXT NULL,
    "IsPendingSync" INTEGER NOT NULL,
    CONSTRAINT "FK_UserRoles_Roles_RoleId" FOREIGN KEY ("RoleId") REFERENCES "Roles" ("Id") ON DELETE CASCADE,
    CONSTRAINT "FK_UserRoles_Users_UserId" FOREIGN KEY ("UserId") REFERENCES "Users" ("Id") ON DELETE CASCADE
);

CREATE UNIQUE INDEX "IX_UserRoles_UserId_RoleId" ON "UserRoles" ("UserId", "RoleId");

CREATE INDEX "IX_UserRoles_RoleId" ON "UserRoles" ("RoleId");

CREATE TABLE "Permissions" (
    "Id" INTEGER NOT NULL CONSTRAINT "PK_Permissions" PRIMARY KEY AUTOINCREMENT,
    "Code" TEXT NOT NULL,
    "DisplayName" TEXT NOT NULL,
    "Description" TEXT NULL,
    "Category" TEXT NOT NULL,
    "IsSystem" INTEGER NOT NULL,
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

CREATE UNIQUE INDEX "IX_Permissions_Code" ON "Permissions" ("Code");

CREATE TABLE "RolePermissions" (
    "Id" INTEGER NOT NULL CONSTRAINT "PK_RolePermissions" PRIMARY KEY AUTOINCREMENT,
    "RoleId" INTEGER NOT NULL,
    "PermissionId" INTEGER NOT NULL,
    "GrantedAt" TEXT NOT NULL,
    "GrantedBy" TEXT NULL,
    "CreatedAt" TEXT NOT NULL,
    "CreatedBy" TEXT NULL,
    "ModifiedAt" TEXT NULL,
    "ModifiedBy" TEXT NULL,
    "IsDeleted" INTEGER NOT NULL,
    "DeletedAt" TEXT NULL,
    "DeletedBy" TEXT NULL,
    "SyncId" TEXT NOT NULL,
    "LastSyncAt" TEXT NULL,
    "IsPendingSync" INTEGER NOT NULL,
    CONSTRAINT "FK_RolePermissions_Permissions_PermissionId" FOREIGN KEY ("PermissionId") REFERENCES "Permissions" ("Id") ON DELETE CASCADE,
    CONSTRAINT "FK_RolePermissions_Roles_RoleId" FOREIGN KEY ("RoleId") REFERENCES "Roles" ("Id") ON DELETE CASCADE
);

CREATE UNIQUE INDEX "IX_RolePermissions_RoleId_PermissionId" ON "RolePermissions" ("RoleId", "PermissionId");

CREATE INDEX "IX_RolePermissions_PermissionId" ON "RolePermissions" ("PermissionId");

CREATE TABLE "UserPermissions" (
    "Id" INTEGER NOT NULL CONSTRAINT "PK_UserPermissions" PRIMARY KEY AUTOINCREMENT,
    "UserId" INTEGER NOT NULL,
    "PermissionId" INTEGER NOT NULL,
    "IsGranted" INTEGER NOT NULL,
    "AssignedAt" TEXT NOT NULL,
    "AssignedBy" TEXT NULL,
    "ExpiresAt" TEXT NULL,
    "CreatedAt" TEXT NOT NULL,
    "CreatedBy" TEXT NULL,
    "ModifiedAt" TEXT NULL,
    "ModifiedBy" TEXT NULL,
    "IsDeleted" INTEGER NOT NULL,
    "DeletedAt" TEXT NULL,
    "DeletedBy" TEXT NULL,
    "SyncId" TEXT NOT NULL,
    "LastSyncAt" TEXT NULL,
    "IsPendingSync" INTEGER NOT NULL,
    CONSTRAINT "FK_UserPermissions_Permissions_PermissionId" FOREIGN KEY ("PermissionId") REFERENCES "Permissions" ("Id") ON DELETE CASCADE,
    CONSTRAINT "FK_UserPermissions_Users_UserId" FOREIGN KEY ("UserId") REFERENCES "Users" ("Id") ON DELETE CASCADE
);

CREATE UNIQUE INDEX "IX_UserPermissions_UserId_PermissionId" ON "UserPermissions" ("UserId", "PermissionId");

CREATE INDEX "IX_UserPermissions_PermissionId" ON "UserPermissions" ("PermissionId");

CREATE TABLE "AuditLogs" (
    "Id" INTEGER NOT NULL CONSTRAINT "PK_AuditLogs" PRIMARY KEY AUTOINCREMENT,
    "EventType" INTEGER NOT NULL,
    "UserId" INTEGER NULL,
    "Username" TEXT NULL,
    "ClientInfo" TEXT NULL,
    "Resource" TEXT NULL,
    "Action" TEXT NOT NULL,
    "Details" TEXT NULL,
    "IsSuccess" INTEGER NOT NULL,
    "ErrorMessage" TEXT NULL,
    "Timestamp" TEXT NOT NULL,
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

CREATE INDEX "IX_AuditLogs_Timestamp" ON "AuditLogs" ("Timestamp");

CREATE INDEX "IX_AuditLogs_UserId" ON "AuditLogs" ("UserId");

CREATE INDEX "IX_AuditLogs_EventType" ON "AuditLogs" ("EventType");
