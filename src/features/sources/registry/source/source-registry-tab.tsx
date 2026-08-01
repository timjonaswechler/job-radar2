import { SourceEditDrawer } from "@/features/sources/edit/source/source-edit-drawer";
import { SourceDetailsDrawer } from "@/features/sources/registry/source/source-details-drawer";
import { SourceRegistryGrid } from "@/features/sources/registry/source/source-registry-grid";
import { useSourceRegistryTab } from "@/features/sources/registry/source/use-source-registry-tab";
import type { DiagnosticIndex } from "@/features/sources/view-model/diagnostics";
import type {
  InstalledSource,
  InstalledProfileWithDefinition,
} from "@/lib/api/sources";

type SourceRegistryTabProps = {
  sources: InstalledSource[];
  profilesByKey: Map<string, InstalledProfileWithDefinition>;
  diagnosticIndex: DiagnosticIndex;
  loading: boolean;
  onAdd: () => void;
  onUpdated?: () => Promise<unknown> | unknown;
};

export function SourceRegistryTab({
  sources,
  profilesByKey,
  diagnosticIndex,
  loading,
  onAdd,
  onUpdated,
}: SourceRegistryTabProps) {
  const state = useSourceRegistryTab({
    sources,
    profilesByKey,
    diagnosticIndex,
  });

  return (
    <>
      <SourceRegistryGrid state={state} loading={loading} onAdd={onAdd} />

      <SourceDetailsDrawer
        row={state.selectedRow}
        profilesByKey={profilesByKey}
        diagnostics={
          state.selectedRow
            ? (diagnosticIndex.bySourceKey.get(state.selectedRow.key) ?? [])
            : []
        }
        open={state.selectedRow !== null}
        onEdit={state.editSource}
        onUpdated={onUpdated}
        onOpenChange={(open) => {
          if (!open) state.closeDetails();
        }}
      />

      <SourceEditDrawer
        source={state.editingSource}
        profilesByKey={profilesByKey}
        open={state.editingSource !== null}
        onUpdated={onUpdated}
        onOpenChange={(open) => {
          if (!open) state.closeEdit();
        }}
      />
    </>
  );
}
