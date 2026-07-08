import test from "node:test"
import assert from "node:assert/strict"

const { isWindowControlPlatform, resolveWindowControlPlatform } = await import(
  "../src/lib/window-control-platform.ts"
)

test("accepts only supported window-control platform overrides", () => {
  assert.equal(isWindowControlPlatform("macos"), true)
  assert.equal(isWindowControlPlatform("windows"), true)
  assert.equal(isWindowControlPlatform("linux"), true)
  assert.equal(isWindowControlPlatform("gnome"), false)
  assert.equal(isWindowControlPlatform("reset"), false)
  assert.equal(isWindowControlPlatform(null), false)
})

test("override wins over detected navigator platform", () => {
  assert.equal(
    resolveWindowControlPlatform({
      navigatorPlatform: "MacIntel",
      userAgent: "Mozilla/5.0 Macintosh",
      override: "windows",
    }),
    "windows"
  )
  assert.equal(
    resolveWindowControlPlatform({
      navigatorPlatform: "Win32",
      userAgent: "Mozilla/5.0 Windows",
      override: "linux",
    }),
    "linux"
  )
})

test("detects macOS from platform or user agent", () => {
  assert.equal(
    resolveWindowControlPlatform({
      navigatorPlatform: "MacIntel",
      userAgent: "",
      override: null,
    }),
    "macos"
  )
  assert.equal(
    resolveWindowControlPlatform({
      navigatorPlatform: "",
      userAgent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0)",
      override: null,
    }),
    "macos"
  )
})

test("detects Windows from platform or user agent", () => {
  assert.equal(
    resolveWindowControlPlatform({
      navigatorPlatform: "Win32",
      userAgent: "",
      override: null,
    }),
    "windows"
  )
  assert.equal(
    resolveWindowControlPlatform({
      navigatorPlatform: "",
      userAgent: "Mozilla/5.0 Windows NT 10.0",
      override: null,
    }),
    "windows"
  )
})

test("falls back to linux for unknown desktop platforms", () => {
  assert.equal(
    resolveWindowControlPlatform({
      navigatorPlatform: "X11",
      userAgent: "Mozilla/5.0 X11 Linux x86_64",
      override: null,
    }),
    "linux"
  )
})
