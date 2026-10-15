-- Plugin persistence (C# Arcana.Plugins/Data/PluginDbContext.cs). C# kept these in a separate
-- PluginDbContext; here they share the application database, still in their own tables.

CREATE TABLE "PluginVersions" (
    "Id" INTEGER NOT NULL CONSTRAINT "PK_PluginVersions" PRIMARY KEY AUTOINCREMENT,
    "PluginId" TEXT NOT NULL,
    "Version" TEXT NOT NULL,
    "InstalledAt" TEXT NOT NULL,
    "BackupPath" TEXT NOT NULL,
    "IsCurrent" INTEGER NOT NULL DEFAULT 0,
    "SizeBytes" INTEGER NOT NULL DEFAULT 0,
    "ReleaseNotes" TEXT NULL,
    "Checksum" TEXT NULL
);
CREATE INDEX "IX_PluginVersions_PluginId" ON "PluginVersions" ("PluginId");
CREATE UNIQUE INDEX "IX_PluginVersions_PluginId_Version" ON "PluginVersions" ("PluginId", "Version");

CREATE TABLE "PluginMetadata" (
    "Id" INTEGER NOT NULL CONSTRAINT "PK_PluginMetadata" PRIMARY KEY AUTOINCREMENT,
    "PluginId" TEXT NOT NULL,
    "Name" TEXT NOT NULL,
    "Version" TEXT NOT NULL,
    "Description" TEXT NULL,
    "Author" TEXT NULL,
    "Type" INTEGER NOT NULL DEFAULT 0,
    "State" INTEGER NOT NULL DEFAULT 0,
    "IconPath" TEXT NULL,
    "InstallPath" TEXT NOT NULL,
    "InstalledAt" TEXT NOT NULL,
    "LastActivatedAt" TEXT NULL,
    "IsBuiltIn" INTEGER NOT NULL DEFAULT 0,
    "DependenciesJson" TEXT NULL,
    "MetadataJson" TEXT NULL,
    "ErrorCount" INTEGER NOT NULL DEFAULT 0,
    "LastErrorAt" TEXT NULL,
    "LastError" TEXT NULL
);
CREATE UNIQUE INDEX "IX_PluginMetadata_PluginId" ON "PluginMetadata" ("PluginId");

CREATE TABLE "PluginHealthSnapshots" (
    "Id" INTEGER NOT NULL CONSTRAINT "PK_PluginHealthSnapshots" PRIMARY KEY AUTOINCREMENT,
    "PluginId" TEXT NOT NULL,
    "CheckedAt" TEXT NOT NULL,
    "HealthState" INTEGER NOT NULL DEFAULT 0,
    "Message" TEXT NULL,
    "ResponseTimeMs" INTEGER NOT NULL DEFAULT 0,
    "MemoryUsageBytes" INTEGER NOT NULL DEFAULT 0,
    "ErrorCount" INTEGER NOT NULL DEFAULT 0,
    "DetailsJson" TEXT NULL
);
CREATE INDEX "IX_PluginHealthSnapshots_PluginId" ON "PluginHealthSnapshots" ("PluginId");
CREATE INDEX "IX_PluginHealthSnapshots_CheckedAt" ON "PluginHealthSnapshots" ("CheckedAt");
