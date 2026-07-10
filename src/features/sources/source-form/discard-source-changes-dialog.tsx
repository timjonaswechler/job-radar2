import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";

type DiscardSourceChangesDialogProps = {
  open: boolean;
  portalContainer?: HTMLElement | null;
  onCancel: () => void;
  onConfirm: () => void;
};

export function DiscardSourceChangesDialog({
  open,
  portalContainer,
  onCancel,
  onConfirm,
}: DiscardSourceChangesDialogProps) {
  return (
    <AlertDialog
      open={open}
      onOpenChange={(nextOpen) => {
        if (!nextOpen) onCancel();
      }}
    >
      <AlertDialogContent portalContainer={portalContainer}>
        <AlertDialogHeader>
          <AlertDialogTitle>Änderungen verwerfen?</AlertDialogTitle>
          <AlertDialogDescription>
            Deine nicht gespeicherten Änderungen an dieser Source gehen verloren.
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel onClick={onCancel}>
            Weiter bearbeiten
          </AlertDialogCancel>
          <AlertDialogAction variant="destructive" onClick={onConfirm}>
            Änderungen verwerfen
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
