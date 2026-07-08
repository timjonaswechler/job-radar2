import { CommandSearchDialog } from "./command-search-dialog";
import { LanguageSwitcher } from "./language-switcher";
import { ThemeSwitcher } from "./theme-switcher";
import { Separator } from "@/components/ui/separator";
import { SidebarTrigger } from "@/components/ui/sidebar";

type AppHeaderProps = {
  title: string;
  windowDragRegionEnabled: boolean;
};

type DragRegionProps = {
  "data-tauri-drag-region"?: string;
};

export function AppHeader({ title, windowDragRegionEnabled }: AppHeaderProps) {
  const dragRegionProps: DragRegionProps = windowDragRegionEnabled
    ? { "data-tauri-drag-region": "" }
    : {};

  return (
    <header className="sticky top-0 z-50 flex h-12 shrink-0 items-center gap-2 overflow-hidden rounded-t-[inherit] border-b bg-background/50 backdrop-blur-md transition-[width,height] ease-linear group-has-data-[collapsible=icon]/sidebar-wrapper:h-12">
      <div className="app-header-content flex h-full w-full min-w-0 items-center">
        <div className="flex items-center gap-1 lg:gap-2">
          <SidebarTrigger className="-ml-1" />
          <Separator
            orientation="vertical"
            className="mx-2 data-[orientation=vertical]:h-4 data-[orientation=vertical]:self-center"
          />
          <CommandSearchDialog />
        </div>
        <div
          className="h-full min-w-4 flex-1 cursor-default select-none"
          aria-hidden="true"
          {...dragRegionProps}
        />
        <div className="flex items-center gap-2">
          <p
            className="hidden cursor-default select-none text-sm font-medium text-muted-foreground md:block"
            {...dragRegionProps}
          >
            {title}
          </p>
          <LanguageSwitcher />
          <ThemeSwitcher />
        </div>
      </div>
    </header>
  );
}
