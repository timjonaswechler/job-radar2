import type { ReactNode } from "react";

type PostingsPageFrameProps = {
  children: ReactNode;
};

export function PostingsPageFrame({ children }: PostingsPageFrameProps) {
  return (
    <div className="flex min-h-0 flex-1 overflow-hidden text-card-foreground shadow-sm">
      {children}
    </div>
  );
}
