import type { ReactNode } from "react";

type PostingsLayoutProps = {
  children: ReactNode;
};

export function PostingsLayout({ children }: PostingsLayoutProps) {
  return (
    <div className="flex min-h-0 flex-1 overflow-hidden text-card-foreground shadow-sm">
      {children}
    </div>
  );
}
