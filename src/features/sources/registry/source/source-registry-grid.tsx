import { PlusIcon } from "lucide-react";

import { DataGridPagination } from "@/components/reui/data-grid/data-grid-pagination";
import { DataGridScrollArea } from "@/components/reui/data-grid/data-grid-scroll-area";
import { DataGridTable } from "@/components/reui/data-grid/data-grid-table";
import { Button } from "@/components/ui/button";
import { RegistryGridShell } from "@/features/sources/registry/shared/registry-grid-shell";
import { RegistrySearchInput } from "@/features/sources/registry/shared/registry-search-input";
import { registryRowHealthClassName } from "@/features/sources/registry/shared/registry-state-indicator";
import { SourceFilterPopover } from "@/features/sources/registry/source/source-filter-popover";
import type { SourceRegistryTabState } from "@/features/sources/registry/source/use-source-registry-tab";

type SourceRegistryGridProps = {
  state: SourceRegistryTabState;
  loading: boolean;
  onAdd: () => void;
};

export function SourceRegistryGrid({
  state,
  loading,
  onAdd,
}: SourceRegistryGridProps) {
  return (
    <RegistryGridShell
      table={state.table}
      recordCount={state.filteredRows.length}
      isLoading={loading}
      loadingMessage="Quellen werden geladen…"
      emptyMessage="Keine Registry-Quellen gefunden."
      title="Registry-Quellen"
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
            label="Quellen suchen"
            name="source-registry-search"
            placeholder="Quellen suchen…"
            clearLabel="Quellensuche leeren"
          />
          <SourceFilterPopover
            selectedStatuses={state.selectedStatuses}
            selectedOrigins={state.selectedOrigins}
            diagnosticsOnly={state.diagnosticsOnly}
            statusCounts={state.statusCounts}
            originCounts={state.originCounts}
            activeFilterCount={state.activeFilterCount}
            onStatusChange={state.toggleStatus}
            onOriginChange={state.toggleOrigin}
            onDiagnosticsOnlyChange={state.setDiagnosticsOnly}
          />
          <Button type="button" onClick={onAdd}>
            <PlusIcon data-icon="inline-start" aria-hidden="true" />
            Quelle hinzufügen
          </Button>
        </>
      }
      pagination={
        <DataGridPagination
          sizes={[5, 10, 25, 50]}
          rowsPerPageLabel="Zeilen pro Seite"
          info="{from}–{to} von {count}"
          previousPageLabel="Vorherige Quellen-Seite"
          nextPageLabel="Nächste Quellen-Seite"
        />
      }
      onRowClick={state.setSelectedRow}
      rowClassName={(row) => registryRowHealthClassName(row.health)}
      className="px-1"
      actionsClassName="items-baseline"
    >
      <DataGridScrollArea>
        <DataGridTable />
      </DataGridScrollArea>
    </RegistryGridShell>
  );
}
