import { PlusIcon } from "lucide-react";

import { DataGridPagination } from "@/components/reui/data-grid/data-grid-pagination";
import { DataGridScrollArea } from "@/components/reui/data-grid/data-grid-scroll-area";
import { DataGridTable } from "@/components/reui/data-grid/data-grid-table";
import { Button } from "@/components/ui/button";
import { ProfileFilterPopover } from "@/features/sources/registry/profile/profile-filter-popover";
import type { ProfileRegistryTabState } from "@/features/sources/registry/profile/use-profile-registry-tab";
import { RegistryGridShell } from "@/features/sources/registry/shared/registry-grid-shell";
import { RegistrySearchInput } from "@/features/sources/registry/shared/registry-search-input";
import { registryRowHealthClassName } from "@/features/sources/registry/shared/registry-state-indicator";

type ProfileRegistryGridProps = {
  state: ProfileRegistryTabState;
  loading: boolean;
  onAdd: () => void;
};

export function ProfileRegistryGrid({
  state,
  loading,
  onAdd,
}: ProfileRegistryGridProps) {
  return (
    <RegistryGridShell
      table={state.table}
      recordCount={state.filteredRows.length}
      isLoading={loading}
      loadingMessage="Profile werden geladen…"
      emptyMessage="Keine Registry-Profile gefunden."
      title="Quellenprofile"
      description={
        <>
          Der Punkt vor dem Namen zeigt den Registry-Zustand. Bei Problemen
          Zeile anklicken, um Details im Drawer zu öffnen.
        </>
      }
      actions={
        <>
          <RegistrySearchInput
            value={state.searchQuery}
            onChange={state.setSearchQuery}
            label="Profile suchen"
            name="profile-registry-search"
            placeholder="Profile suchen…"
            clearLabel="Profilsuche leeren"
          />
          <ProfileFilterPopover
            selectedKinds={state.selectedKinds}
            selectedOrigins={state.selectedOrigins}
            diagnosticsOnly={state.diagnosticsOnly}
            kindCounts={state.kindCounts}
            originCounts={state.originCounts}
            activeFilterCount={state.activeFilterCount}
            onKindChange={state.toggleKind}
            onOriginChange={state.toggleOrigin}
            onDiagnosticsOnlyChange={state.setDiagnosticsOnly}
          />
          <Button type="button" onClick={onAdd}>
            <PlusIcon data-icon="inline-start" aria-hidden="true" />
            Profil hinzufügen
          </Button>
        </>
      }
      pagination={
        <DataGridPagination
          sizes={[5, 10, 25, 50]}
          rowsPerPageLabel="Zeilen pro Seite"
          info="{from}–{to} von {count}"
          previousPageLabel="Vorherige Profil-Seite"
          nextPageLabel="Nächste Profil-Seite"
        />
      }
      onRowClick={state.setSelectedRow}
      rowClassName={(row) => registryRowHealthClassName(row.health)}
      actionsClassName="items-center"
    >
      <DataGridScrollArea>
        <DataGridTable />
      </DataGridScrollArea>
    </RegistryGridShell>
  );
}
