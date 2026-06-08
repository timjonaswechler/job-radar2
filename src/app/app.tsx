import { useState } from "react";

import { AppLayout } from "@/components/layout/app-layout";
import type { AppPage } from "@/lib/navigation";
import { navigationItems, pageMeta } from "@/lib/navigation";
import { HomePage } from "@/pages/home-page";

export function App() {
  const [activePage, setActivePage] = useState<AppPage>("home");
  const meta = pageMeta[activePage];

  return (
    <AppLayout
      activePage={activePage}
      navigationItems={navigationItems}
      title={meta.title}
      subtitle={meta.subtitle}
      onPageChange={setActivePage}
    >
      <HomePage />
    </AppLayout>
  );
}
