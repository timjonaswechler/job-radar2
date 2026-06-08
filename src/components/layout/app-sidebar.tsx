import type { AppPage, NavigationItem } from "@/lib/navigation";
import { cn } from "@/lib/utils";

type AppSidebarProps = {
  activePage: AppPage;
  items: NavigationItem[];
  onPageChange: (page: AppPage) => void;
};

export function AppSidebar({ activePage, items, onPageChange }: AppSidebarProps) {
  return (
    <aside className="hidden w-72 shrink-0 border-r bg-card lg:block">
      <div className="flex h-20 items-center border-b px-6">
        <div className="grid gap-0.5">
          <span className="text-lg font-semibold tracking-tight">App Shell</span>
          <span className="text-xs text-muted-foreground">Tauri + React + SQLite</span>
        </div>
      </div>

      <nav className="grid gap-1 p-3" aria-label="Hauptnavigation">
        {items.map((item) => {
          const Icon = item.icon;
          const active = item.id === activePage;

          return (
            <button
              key={item.id}
              className={cn(
                "flex w-full items-start gap-3 rounded-lg px-3 py-3 text-left text-sm transition-colors",
                active
                  ? "bg-primary text-primary-foreground shadow-xs"
                  : "text-muted-foreground hover:bg-accent hover:text-accent-foreground",
              )}
              type="button"
              onClick={() => onPageChange(item.id)}
            >
              <Icon className="mt-0.5 size-4 shrink-0" aria-hidden="true" />
              <span className="grid gap-0.5">
                <span className="font-medium">{item.label}</span>
                <span
                  className={cn(
                    "text-xs",
                    active ? "text-primary-foreground/75" : "text-muted-foreground",
                  )}
                >
                  {item.description}
                </span>
              </span>
            </button>
          );
        })}
      </nav>
    </aside>
  );
}
