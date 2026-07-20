import type { DetailStrategy, DiscoveryStrategy } from "@/lib/api/sources";

const literal = (value: string) => ({ type: "const" as const, value });

export function minimalDiscoveryStrategy(key: string): DiscoveryStrategy {
  return {
    key,
    fetch: { mode: "http", url: "https://example.test/jobs", timeoutMs: 1_000 },
    parse: { type: "json" },
    select: { type: "document" },
    extract: {
      fields: {
        title: literal("Example role"),
        company: literal("Example company"),
        url: literal("https://example.test/jobs/1"),
      },
    },
  };
}

export function minimalDetailStrategy(key: string): DetailStrategy {
  return {
    key,
    fetch: { mode: "http", url: "https://example.test/jobs/1", timeoutMs: 1_000 },
    parse: { type: "json" },
    select: { type: "document" },
    extract: { fields: { descriptionText: literal("Example description") } },
  };
}
