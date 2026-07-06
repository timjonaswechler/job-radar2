import type { ReactNode } from "react";

type SourcesPageFrameProps = {
  children: ReactNode;
};

export function SourcesPageFrame({ children }: SourcesPageFrameProps) {
  return <div className="grid gap-4 p-2">{children}</div>;
}
