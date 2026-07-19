import type {
  PostingPreparationProgressViewModel,
  PostingPreparationTaskStatus,
} from "@/features/postings/view-model/posting-item-view-model";
import { cn } from "@/lib/utils";

type PostingPreparationStepperProps = {
  progress: PostingPreparationProgressViewModel;
};

export function PostingPreparationStepper({
  progress,
}: PostingPreparationStepperProps) {
  return (
    <div
      role="group"
      className="mt-2 flex min-w-0 flex-col gap-1.5"
      aria-label={progress.accessibleLabel}
    >
      <div className="flex items-center justify-between gap-2 text-[0.625rem] leading-none">
        <span className="truncate font-medium">{progress.leadLabel}</span>
        <span className="shrink-0 text-muted-foreground">
          {progress.completedCount}/{progress.applicableCount} erledigt
        </span>
      </div>

      <div className="grid grid-cols-5 gap-1" aria-hidden="true">
        {progress.steps.map((step) => (
          <span
            key={step.task}
            title={`${step.label}: ${step.statusLabel}`}
            className={cn(
              "h-1.5 rounded-full",
              getSegmentClassName(step.status),
            )}
          />
        ))}
      </div>
    </div>
  );
}

function getSegmentClassName(status: PostingPreparationTaskStatus) {
  if (status === "completed") return "bg-primary";
  if (status === "in_progress") return "bg-primary/40";
  if (status === "not_applicable") return "bg-muted-foreground/20";
  return "bg-muted";
}
