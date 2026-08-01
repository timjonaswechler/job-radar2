export * from "./profiles";
export * from "./installed";
export * from "./detection";
export * from "./live-check";

import { invoke } from "@tauri-apps/api/core";
import type {
  CreateSourceDraft,
  ReviseSourceDefinition,
  InactiveSourceStatus,
} from "./profiles";
import { decodeInstalledSource } from "./installed";

export async function createSource(draft: CreateSourceDraft) {
  return decodeInstalledSource(
    await invoke<unknown>("create_source", { draft }),
  );
}
export async function updateSource(revision: ReviseSourceDefinition) {
  return decodeInstalledSource(
    await invoke<unknown>("update_source", { revision }),
  );
}
export async function setSourceInactive(
  sourceKey: string,
  status: InactiveSourceStatus,
) {
  return decodeInstalledSource(
    await invoke<unknown>("set_source_inactive", { sourceKey, status }),
  );
}

export function sourceCommandErrorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof (error as { message?: unknown }).message === "string"
  ) {
    return (error as { message: string }).message;
  }
  return String(error);
}
