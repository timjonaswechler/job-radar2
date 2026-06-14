import type { AdapterMetadata } from "@/lib/api/sources";

const adapterPriorityByKey = new Map<string, number>([
  ["declarative_endpoint_inventory", 10],
  ["declarative_sitemap_inventory", 20],
  ["declarative_browser_inventory", 30],
  ["stepstone_search", 80],
  ["indeed_search", 90],
]);

const adapterCategoryLabels: Record<AdapterMetadata["category"], string> = {
  job_board: "Job-Portal",
  generic: "Generische Laufzeit",
  browser: "Browser-Laufzeit",
};

const adapterRiskLabels: Record<AdapterMetadata["riskLevel"], string> = {
  stable: "stabil",
  fragile: "fragil",
  restricted: "eingeschränkt",
};

export type AdapterDisplay = {
  key: string;
  name: string;
  label: string;
  registered: boolean;
};

export function sortAdaptersByUserFacingPriority(adapters: AdapterMetadata[]) {
  return [...adapters].sort((left, right) => {
    const leftPriority = adapterPriorityByKey.get(left.key) ?? 125;
    const rightPriority = adapterPriorityByKey.get(right.key) ?? 125;

    if (leftPriority !== rightPriority) return leftPriority - rightPriority;
    return left.name.localeCompare(right.name, "de");
  });
}

export function formatAdapterOptionLabel(adapter: AdapterMetadata) {
  return `[${adapterCategoryLabels[adapter.category]}] ${adapter.name} (${adapter.key})`;
}

export function formatAdapterCategory(adapter: AdapterMetadata) {
  return adapterCategoryLabels[adapter.category];
}

export function formatAdapterRisk(adapter: AdapterMetadata) {
  return adapterRiskLabels[adapter.riskLevel];
}

export function getAdapterDisplay(
  adapterKey: string,
  adapter: AdapterMetadata | null | undefined,
): AdapterDisplay {
  if (!adapter) {
    return {
      key: adapterKey,
      name: "Unregistrierter Adapter",
      label: `${adapterKey} (unregistriert)`,
      registered: false,
    };
  }

  return {
    key: adapter.key,
    name: adapter.name,
    label: formatAdapterOptionLabel(adapter),
    registered: true,
  };
}
