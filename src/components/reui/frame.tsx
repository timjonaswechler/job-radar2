import * as React from "react";

import { cn } from "@/lib/utils";

export type FrameProps = React.ComponentProps<"section"> & {
  title?: React.ReactNode;
  description?: React.ReactNode;
  action?: React.ReactNode;
  footer?: React.ReactNode;
};

export function Frame({
  title,
  description,
  action,
  footer,
  className,
  children,
  ...props
}: FrameProps) {
  return (
    <section
      data-slot="reui-frame"
      className={cn(
        "rounded-xl border bg-card text-card-foreground shadow-xs",
        className,
      )}
      {...props}
    >
      {(title || description || action) && (
        <header className="flex flex-col gap-4 border-b p-6 sm:flex-row sm:items-start sm:justify-between">
          <div className="grid gap-1.5">
            {title ? <h2 className="font-semibold tracking-tight">{title}</h2> : null}
            {description ? (
              <p className="text-sm text-muted-foreground">{description}</p>
            ) : null}
          </div>
          {action ? <div className="shrink-0">{action}</div> : null}
        </header>
      )}
      <div className="p-6">{children}</div>
      {footer ? <footer className="border-t p-6">{footer}</footer> : null}
    </section>
  );
}
