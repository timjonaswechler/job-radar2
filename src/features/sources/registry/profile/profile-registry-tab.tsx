import { ProfileDetailsDrawer } from "@/features/sources/registry/profile/profile-details-drawer";
import { ProfileRegistryGrid } from "@/features/sources/registry/profile/profile-registry-grid";
import { useProfileRegistryTab } from "@/features/sources/registry/profile/use-profile-registry-tab";
import type { DiagnosticIndex } from "@/features/sources/view-model/diagnostics";
import type { RegistrySourceProfile } from "@/lib/api/sources";

type ProfileRegistryTabProps = {
  profiles: RegistrySourceProfile[];
  diagnosticIndex: DiagnosticIndex;
  loading: boolean;
  onAdd: () => void;
};

export function ProfileRegistryTab({
  profiles,
  diagnosticIndex,
  loading,
  onAdd,
}: ProfileRegistryTabProps) {
  const state = useProfileRegistryTab({ profiles, diagnosticIndex });

  return (
    <>
      <ProfileRegistryGrid state={state} loading={loading} onAdd={onAdd} />

      <ProfileDetailsDrawer
        row={state.selectedRow}
        diagnostics={
          state.selectedRow
            ? (diagnosticIndex.byProfileKey.get(state.selectedRow.key) ?? [])
            : []
        }
        open={state.selectedRow !== null}
        onOpenChange={(open) => {
          if (!open) state.closeDetails();
        }}
      />
    </>
  );
}
