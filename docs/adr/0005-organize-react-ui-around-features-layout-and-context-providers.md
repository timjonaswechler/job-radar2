# Organize the React UI around features, shared layout, and context providers

Job Radar's React UI in `apps/web` will move toward a feature-based structure with a small set of shared layout and provider modules. Domain screens such as dashboard, Stellenanzeigen-Inbox, Bewerbungen, Suchanfragen, Jobquellen, Suchläufe, Erinnerungen, Mails, Daten, and Einstellungen should live under `src/features/<feature>/`, while reusable primitives and app chrome remain shared. Global UI state such as theme, locale/formatting, and command search should be provided from `src/context/` instead of being embedded in sidebar or page components. Static app-shell choices such as sidebar variant, content width, and sticky header should stay as constants instead of becoming user-changeable layout context. This keeps Job Radar's domain slices navigable and lets the app shell evolve independently from feature implementation.

The target structure for `apps/web/src` is:

```txt
src/
  context/
    theme-provider.tsx - done
    theme-provider-context.ts - done
    locale-provider.tsx - done
    locale-provider-context.ts - done
    command-search-provider.tsx - done
    command-search-provider-context.ts - done
  components/
    layout/
      app-layout.tsx - done
      app-header.tsx - done
      app-sidebar.tsx - done
      nav-main.tsx - done
    command-search-dialog.tsx - done
    language-switcher.tsx - done
    theme-switcher.tsx - done
    ui/
    reui/
  features/
    dashboard/
    stellenanzeigen-inbox/
    bewerbungen/
    suchanfragen/
    jobquellen/
    suchlaeufe/
    erinnerungen/
    mails/
    daten/
    einstellungen/
  lib/
    i18n/
    api/
    app-settings.ts
    theme.ts
  navigation/
    app-routes.tsx - done
    command-search-items.ts - done
    path.ts - done
    sidebar/
      sidebar-items.ts - done
  hooks/
  config/
  styles/
```

The web app will keep its existing route registry for now instead of adopting TanStack Router immediately. Route entries should become thin mappings from path to feature component, similar to file-route wrappers in `shadcn-admin`, but without introducing generated routing until Job Radar needs nested routes, typed URL search state, or route-level loaders. `src/pages` is a transitional location for the current route components; after the domain screens have moved into `src/features/<feature>/`, the pages directory should be removed. Non-domain route fallbacks such as a not-found page should move to shared app/navigation code rather than becoming a feature.

Shared app chrome will follow the `shadcn-admin` pattern: sidebar data is declarative, command search consumes a curated command index, and global providers wrap the app once near `main.tsx`/`App.tsx`. Sidebar data belongs in `src/navigation/sidebar/sidebar-items.ts` because it is part of app navigation, not a private layout data file. Header controls such as command search, language switching, and theme switching are rendered by the app header but remain standalone shared components outside `components/layout`. `AppLayout` should remain route-agnostic: `App.tsx`/`navigation` decides which route is active, then passes the current title and page children into the layout. `AppHeader` stays a closed app-chrome component for now, with the page title as its only dynamic input instead of route-specific slots. Command search is intentionally limited to pages, sections within pages, and app-level actions; entity/table search such as individual Stellenanzeigen or Bewerbungen remains inside the relevant table or feature. Layout configuration remains static unless Job Radar later needs user-configurable app-shell preferences. The DateSelector localization logic currently demonstrated in `src/components/examples/c-date-selector-4.tsx` will be extracted into `src/lib/i18n/date-selector.ts` and limited to Job Radar's supported languages (`de`, `en`) until more languages are intentionally added. Language metadata such as label, flag, date format, and week start should also be centralized in the i18n layer and exposed through a locale provider for app-wide formatting.

This decision intentionally separates shared UI primitives from feature code: `src/components/ui` remains for shadcn-style primitives, `src/components/reui` remains for larger reusable components such as DataGrid and DateSelector, and feature-specific components stay inside their feature directory. The duplicate `components/ui/reui` area should be consolidated or removed once imports have been migrated.

The migration should happen in small commits that keep the app buildable after each step: introduce theme/locale/command-search context providers, move global command search, reorganize sidebar/app chrome, extract DateSelector i18n, move pages into features one domain slice at a time, then clean up duplicate reusable component directories. Persistent per-page UI state such as resizable panel sizes should be handled with focused hooks and storage keys, not with a broad layout provider. After each slice, run the web typecheck, lint, and build commands.
