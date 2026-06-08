import { Database, Home, type LucideIcon } from "lucide-react";

export type AppPage = "home";

export type NavigationItem = {
  id: AppPage;
  label: string;
  description: string;
  icon: LucideIcon;
};

export const navigationItems: NavigationItem[] = [
  {
    id: "home",
    label: "Start",
    description: "App-Shell und Infrastruktur",
    icon: Home,
  },
];

export const pageMeta: Record<AppPage, { title: string; subtitle: string }> = {
  home: {
    title: "App-Shell",
    subtitle: "Neutraler Startpunkt mit UI-System, Layout und lokaler SQLite-Datenbank.",
  },
};

export const infrastructureItems = [
  {
    label: "React + Vite",
    description: "Frontend-Shell mit TypeScript und schnellem Dev-Server.",
  },
  {
    label: "shadcn/ReUI-Struktur",
    description: "Copy-and-own Komponenten, Feature-Slices und Pages.",
  },
  {
    label: "SQLite",
    description: "Lokale Datenbank im Tauri App-Data-Verzeichnis.",
    icon: Database,
  },
];
