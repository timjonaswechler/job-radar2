import type { ReactNode } from "react";

import type { Table } from "@tanstack/react-table";

import { DataGrid } from "@/components/reui/data-grid/data-grid";
import {
  Frame,
  FrameDescription,
  FrameFooter,
  FrameHeader,
  FramePanel,
  FrameTitle,
} from "@/components/reui/frame";
import { cn } from "@/lib/utils";

type RegistryGridShellProps<TRow extends object> = {
  table: Table<TRow>;
  recordCount: number;
  isLoading: boolean;
  loadingMessage: ReactNode;
  emptyMessage: ReactNode;
  title: ReactNode;
  description: ReactNode;
  actions: ReactNode;
  children: ReactNode;
  pagination: ReactNode;
  onRowClick: (row: TRow) => void;
  rowClassName: (row: TRow) => string;
  className?: string;
  actionsClassName?: string;
};

export function RegistryGridShell<TRow extends object>({
  table,
  recordCount,
  isLoading,
  loadingMessage,
  emptyMessage,
  title,
  description,
  actions,
  children,
  pagination,
  onRowClick,
  rowClassName,
  className,
  actionsClassName,
}: RegistryGridShellProps<TRow>) {
  return (
    <DataGrid
      table={table}
      recordCount={recordCount}
      isLoading={isLoading}
      loadingMessage={loadingMessage}
      emptyMessage={emptyMessage}
      onRowClick={onRowClick}
      tableClassNames={{ bodyRow: rowClassName }}
      tableLayout={{
        columnsPinnable: true,
        columnsResizable: false,
        columnsMovable: true,
        columnsVisibility: true,
      }}
    >
      <Frame className={cn("w-full", className)} stacked dense>
        <FrameHeader className="gap-3 sm:flex-row sm:items-start sm:justify-between">
          <div className="grid gap-1.5">
            <FrameTitle>{title}</FrameTitle>
            <FrameDescription>{description}</FrameDescription>
          </div>
          <div
            className={cn(
              "flex flex-wrap gap-2.5",
              actionsClassName,
            )}
          >
            {actions}
          </div>
        </FrameHeader>
        <FramePanel className="p-0 shadow-none">{children}</FramePanel>
        <FrameFooter className="py-1.5 pr-2 pl-2.5">
          {pagination}
        </FrameFooter>
      </Frame>
    </DataGrid>
  );
}
