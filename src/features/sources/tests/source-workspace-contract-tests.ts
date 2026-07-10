import assert from "node:assert/strict";

import {
  customRegistryFolderEntries,
  customRegistryFoldersDescription,
  CustomRegistryFoldersCard,
} from "@/features/sources/workspace/custom-registry-folders-card";
import type { DatabaseInfo } from "@/lib/api/database";

const databaseInfo: DatabaseInfo = {
  appDataDir: "/tmp/job-radar",
  databasePath: "/tmp/job-radar/job-radar.sqlite",
  sourceProfilesDir: "/tmp/job-radar/source-profiles",
  sourcesDir: "/tmp/job-radar/sources",
  initializedAt: null,
  sqliteVersion: "3.50.0",
};

assert.equal(typeof CustomRegistryFoldersCard, "function");
assert.deepEqual(customRegistryFolderEntries(databaseInfo), [
  {
    label: "Eigene Sources",
    pattern: "sources/*.json",
    path: "/tmp/job-radar/sources",
  },
  {
    label: "Eigene Source Profiles",
    pattern: "source-profiles/*.json",
    path: "/tmp/job-radar/source-profiles",
  },
]);
assert.match(customRegistryFoldersDescription, /eigene JSON-Dateien/i);
