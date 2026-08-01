import { describe, expect, it } from "vitest";
import { decodeInventory } from "./installed";
import { sourceCommandErrorMessage } from "./index";

const source = {
  origin: "custom",
  fileName: "example.json",
  document: {
    schemaVersion: 3,
    key: "example",
    name: "Example",
    status: "draft",
    sourceConfig: {},
    selectedAccessPath: {
      type: "profile_access_path",
      profileKey: "greenhouse",
      pathKey: "boards_api",
    },
  },
  validationState: {
    sourceKey: "example",
    state: "valid",
    canCompile: true,
    canExecute: false,
  },
  resolved: { accessPathName: "Boards API", capabilities: ["discovery"] },
};
const profile = {
  origin: "built_in",
  admission: "admitted",
  fileName: "greenhouse.json",
  definition: {
    key: "greenhouse",
    name: "Greenhouse",
    kind: "recruiting_system",
    support: { level: "stable" },
    accessPaths: [{
      key: "boards_api",
      name: "Boards API",
      discovery: {
        policy: { type: "first_accepted" },
        strategies: [{ key: "boards" }],
      },
    }],
  },
};
const inventory = {
  profiles: { profiles: [profile], diagnostics: [] },
  sources: [source],
  diagnostics: [],
};

describe("installed Source transport", () => {
  it("accepts intentional views and rejects raw owner internals", () => {
    expect(decodeInventory(inventory).sources[0]?.fileName).toBe(
      "example.json",
    );
    for (const forbidden of [
      "path",
      "generation",
      "compileOutcome",
      "effectiveProfile",
      "plan",
    ]) {
      expect(() =>
        decodeInventory({
          ...inventory,
          sources: [{ ...source, [forbidden]: "secret" }],
        }),
      ).toThrow(/forbidden/);
    }
  });

  it.each([
    [
      "missing document key",
      { ...source, document: { ...source.document, key: undefined } },
    ],
    [
      "invalid status",
      { ...source, document: { ...source.document, status: "invalid" } },
    ],
    [
      "incomplete selection",
      {
        ...source,
        document: {
          ...source.document,
          selectedAccessPath: {
            type: "profile_access_path",
            profileKey: "greenhouse",
          },
        },
      },
    ],
    [
      "mismatched validation key",
      {
        ...source,
        validationState: { ...source.validationState, sourceKey: "other" },
      },
    ],
    [
      "invalid validation boolean",
      {
        ...source,
        validationState: { ...source.validationState, canCompile: "yes" },
      },
    ],
    [
      "invalid resolved capabilities",
      { ...source, resolved: { ...source.resolved, capabilities: [42] } },
    ],
  ])("rejects %s", (_label, invalidSource) => {
    expect(() =>
      decodeInventory({ ...inventory, sources: [invalidSource] }),
    ).toThrow(/invalid/);
  });

  it("rejects malformed Profiles, Support metadata, Access Paths, and Diagnostics before feature code receives them", () => {
    expect(() =>
      decodeInventory({
        ...inventory,
        profiles: {
          profiles: [{ ...profile, definition: { key: "greenhouse" } }],
          diagnostics: [],
        },
      }),
    ).toThrow(/Profile definition/);
    expect(() =>
      decodeInventory({
        ...inventory,
        profiles: {
          profiles: [{
            ...profile,
            definition: {
              ...profile.definition,
              kind: "ats_guess",
            },
          }],
          diagnostics: [],
        },
      }),
    ).toThrow(/Profile definition/);
    expect(() =>
      decodeInventory({
        ...inventory,
        profiles: {
          profiles: [{
            ...profile,
            definition: {
              ...profile.definition,
              support: { level: "stable", knownIssues: {} },
            },
          }],
          diagnostics: [],
        },
      }),
    ).toThrow(/Support Metadata/);
    expect(() =>
      decodeInventory({
        ...inventory,
        profiles: {
          profiles: [{
            ...profile,
            definition: {
              ...profile.definition,
              detection: {
                policy: { type: "all_required" },
                strategies: [{ key: "url" }],
                evidence: {},
              },
            },
          }],
          diagnostics: [],
        },
      }),
    ).toThrow(/Detection evidence/);
    expect(() =>
      decodeInventory({
        ...inventory,
        profiles: {
          profiles: [{
            ...profile,
            definition: {
              ...profile.definition,
              accessPaths: [{ key: "boards_api", name: "Boards API", discovery: { strategies: {} } }],
            },
          }],
          diagnostics: [],
        },
      }),
    ).toThrow(/Strategy Set/);
    expect(() =>
      decodeInventory({
        ...inventory,
        sources: [{
          ...source,
          document: {
            ...source.document,
            accessPaths: [{ key: "boards_api", discovery: { strategies: {} } }],
          },
        }],
      }),
    ).toThrow(/direct Discovery Strategy Set/);
    expect(() =>
      decodeInventory({
        ...inventory,
        diagnostics: [{ code: "missing-contract" }],
      }),
    ).toThrow(/diagnostics/);
  });

  it("preserves typed Tauri mutation error messages", () => {
    expect(
      sourceCommandErrorMessage({
        kind: "duplicate",
        message: "Source `example` already exists",
      }),
    ).toBe("Source `example` already exists");
  });
});
