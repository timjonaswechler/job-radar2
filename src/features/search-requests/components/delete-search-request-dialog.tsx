import { DeleteConfirmDialog } from "@/features/sources/registry/delete-confirm-dialog";
import type { SearchRequestTableRow } from "@/features/search-requests/model/search-request-row-model";

type DeleteSearchRequestDialogProps = {
  row: SearchRequestTableRow | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onConfirm: () => Promise<void>;
};

export function DeleteSearchRequestDialog({
  row,
  open,
  onOpenChange,
  onConfirm,
}: DeleteSearchRequestDialogProps) {
  return (
    <DeleteConfirmDialog
      open={open}
      onOpenChange={onOpenChange}
      title="Search Request löschen?"
      description={
        row
          ? `${row.title} wird dauerhaft gelöscht. Bestehende Search Runs oder Postings werden dadurch nicht automatisch entfernt.`
          : "Diese Search Request wird dauerhaft gelöscht."
      }
      confirmLabel="Search Request löschen"
      onConfirm={onConfirm}
    />
  );
}
