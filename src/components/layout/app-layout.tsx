import * as React from "react";

import { AppHeader } from "@/components/layout/app-header";
import { AppSidebar } from "@/components/layout/app-sidebar";
import { Button } from "@/components/ui/button";
import type { AppPage, NavigationItem } from "@/lib/navigation";
import { cn } from "@/lib/utils";

type AppLayoutProps = {
  activePage: AppPage;
  children: React.ReactNode;
  navigationItems: NavigationItem[];
  subtitle: string;
  title: string;
  onPageChange: (page: AppPage) => void;
};

export function AppLayout({
  activePage,
  children,
  navigationItems,
  subtitle,
  title,
  onPageChange,
}: AppLayoutProps) {
  return (
    <div className="flex min-h-svh bg-muted/40 text-foreground">
      <AppSidebar
        activePage={activePage}
        items={navigationItems}
        onPageChange={onPageChange}
      />
      <div className="flex min-w-0 flex-1 flex-col">
        <AppHeader title={title} subtitle={subtitle} />
        <nav className="flex gap-2 overflow-x-auto border-b bg-card px-4 py-3 lg:hidden">
          {navigationItems.map((item) => (
            <Button
              key={item.id}
              className={cn("shrink-0", item.id === activePage && "shadow-sm")}
              variant={item.id === activePage ? "default" : "outline"}
              size="sm"
              onClick={() => onPageChange(item.id)}
            >
              {item.label}
            </Button>
          ))}
        </nav>
        <main className="flex-1 p-4 sm:p-6 lg:p-8">{children}</main>
      </div>
    </div>
  );
}
