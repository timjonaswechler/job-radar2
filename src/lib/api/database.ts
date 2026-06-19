import { invoke } from "@tauri-apps/api/core";

export type DatabaseInfo = {
  appDataDir: string;
  databasePath: string;
  sourceProfilesDir: string;
  sourcesDir: string;
  initializedAt: string | null;
  sqliteVersion: string;
};

export function getDatabaseInfo() {
  return invoke<DatabaseInfo>("get_database_info");
}
