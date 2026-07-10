import assert from "node:assert/strict";
import type { MouseEvent } from "react";
import { AppLink } from "@/app/navigation/app-link";
import { appRoutes, getAppRoute } from "@/app/navigation/app-routes";
import { commandSearchItems } from "@/app/navigation/command-search-items";
import {
  getNavigationItem,
  navigationManifest,
  sidebarNavigationGroups,
} from "@/app/navigation/navigation-manifest";
import { isAppPathActive, navigateTo } from "@/app/navigation/path";

const manifestIds = navigationManifest.map((item) => item.id);
const manifestPaths: readonly string[] = navigationManifest.map(
  (item) => item.path,
);

assert.equal(
  new Set(manifestIds).size,
  manifestIds.length,
  "navigation manifest ids must be unique",
);
assert.equal(
  new Set(manifestPaths).size,
  manifestPaths.length,
  "navigation manifest paths must be unique",
);
assert.equal(
  manifestPaths.includes("/bewerbungen"),
  false,
  "the dead applications path must not be registered",
);

assert.deepEqual(
  appRoutes.map((route) => route.id),
  manifestIds,
  "app routes must be projected from the navigation manifest",
);

for (const group of sidebarNavigationGroups) {
  for (const item of group.items) {
    assert.equal(
      getNavigationItem(item.id),
      item,
      `sidebar item ${item.id} must be the canonical manifest entry`,
    );
    assert.notEqual(
      getAppRoute(item.path).id,
      "not-found",
      `sidebar path ${item.path} must resolve to an app route`,
    );
  }
}

for (const item of commandSearchItems) {
  const manifestItem = getNavigationItem(item.navigationId);
  assert.equal(
    item.url,
    manifestItem.path,
    `command item ${item.id} must use its manifest path`,
  );
  assert.notEqual(
    getAppRoute(item.url).id,
    "not-found",
    `command path ${item.url} must resolve to an app route`,
  );
}

assert.equal(getAppRoute("/").id, "overview");
assert.equal(getAppRoute("/postings/inbox").id, "postings");
assert.equal(getAppRoute("/sources/").id, "sources");
assert.equal(getAppRoute("/sources-extra").id, "not-found");
assert.equal(getAppRoute("/missing").id, "not-found");

assert.equal(isAppPathActive("/", "/"), true);
assert.equal(isAppPathActive("/postings", "/postings"), true);
assert.equal(isAppPathActive("/postings/inbox", "/postings"), true);
assert.equal(isAppPathActive("/postings-extra", "/postings"), false);
assert.equal(isAppPathActive("/sources", "/"), false);

const renderedLink = AppLink({ href: "/sources", children: "Sources" });
assert.equal(renderedLink.type, "a");
assert.equal(renderedLink.props.href, "/sources");
assert.equal(renderedLink.props.children, "Sources");

const originalWindowDescriptor = Object.getOwnPropertyDescriptor(
  globalThis,
  "window",
);
const pushedUrls: string[] = [];
let routeEvents = 0;
let scrollCalls = 0;
const locationState = {
  href: "http://app.local/",
  origin: "http://app.local",
  protocol: "http:",
  host: "app.local",
  pathname: "/",
  search: "",
  hash: "",
};

Object.defineProperty(globalThis, "window", {
  configurable: true,
  value: {
    location: locationState,
    history: {
      pushState: (_state: unknown, _unused: string, url: string) => {
        pushedUrls.push(url);
      },
    },
    dispatchEvent: () => {
      routeEvents += 1;
      return true;
    },
    scrollTo: () => {
      scrollCalls += 1;
    },
  },
});

try {
  for (const nativeClick of [
    createClickEvent({ ctrlKey: true }),
    createClickEvent({ metaKey: true }),
    createClickEvent({ shiftKey: true }),
    createClickEvent({ altKey: true }),
    createClickEvent({ button: 1 }),
    createClickEvent({ target: "_blank" }),
    createClickEvent({ download: true }),
    createClickEvent({ href: "https://example.com/sources" }),
    createClickEvent({ href: "http://app.local/#main-content" }),
  ]) {
    renderedLink.props.onClick(nativeClick.event);
    assert.equal(
      nativeClick.prevented,
      false,
      "AppLink must preserve native behavior for non-SPA clicks",
    );
  }

  const internalClick = createClickEvent({});
  renderedLink.props.onClick(internalClick.event);
  assert.equal(internalClick.prevented, true);
  assert.deepEqual(pushedUrls, ["/sources"]);
  assert.equal(routeEvents, 1);
  assert.equal(scrollCalls, 1);

  navigateTo("/?tab=profiles#registry");
  assert.deepEqual(pushedUrls, ["/sources", "/?tab=profiles#registry"]);
  assert.equal(routeEvents, 2);
  assert.equal(
    scrollCalls,
    1,
    "query-only navigation must not reset page scroll",
  );

  Object.assign(locationState, {
    href: "tauri://localhost/",
    origin: "null",
    protocol: "tauri:",
    host: "localhost",
  });
  const tauriLink = AppLink({ href: "/settings", children: "Settings" });
  const tauriClick = createClickEvent({
    href: "tauri://localhost/settings",
  });
  tauriLink.props.onClick(tauriClick.event);
  assert.equal(tauriClick.prevented, true);
  assert.equal(pushedUrls[pushedUrls.length - 1], "/settings");

  const otherTauriHostClick = createClickEvent({
    href: "tauri://external/settings",
  });
  tauriLink.props.onClick(otherTauriHostClick.event);
  assert.equal(otherTauriHostClick.prevented, false);
  assert.equal(pushedUrls[pushedUrls.length - 1], "/settings");
} finally {
  if (originalWindowDescriptor) {
    Object.defineProperty(globalThis, "window", originalWindowDescriptor);
  } else {
    Reflect.deleteProperty(globalThis, "window");
  }
}

function createClickEvent({
  altKey = false,
  button = 0,
  ctrlKey = false,
  download = false,
  href = "http://app.local/sources",
  metaKey = false,
  shiftKey = false,
  target,
}: {
  altKey?: boolean;
  button?: number;
  ctrlKey?: boolean;
  download?: boolean;
  href?: string;
  metaKey?: boolean;
  shiftKey?: boolean;
  target?: string;
}) {
  const click = {
    prevented: false,
    event: undefined as unknown as MouseEvent<HTMLAnchorElement>,
  };
  click.event = {
    altKey,
    button,
    ctrlKey,
    currentTarget: {
      getAttribute: (name: string) => (name === "target" ? target ?? null : null),
      hasAttribute: (name: string) => name === "download" && download,
      href,
    },
    defaultPrevented: false,
    metaKey,
    preventDefault: () => {
      click.prevented = true;
    },
    shiftKey,
  } as unknown as MouseEvent<HTMLAnchorElement>;

  return click;
}
