import { describe, expect, it } from "vitest"

import { backgroundImageUrl, formatBytes, formatClock, formatEta, formatSpeed } from "@/lib/utils"

describe("formatBytes", () => {
  it("renders zero and negative values as 0 B", () => {
    expect(formatBytes(0)).toBe("0 B")
    expect(formatBytes(-5)).toBe("0 B")
    expect(formatBytes(Number.NaN)).toBe("0 B")
  })

  it("scales with 1024-based units", () => {
    expect(formatBytes(512)).toBe("512 B")
    expect(formatBytes(1024)).toBe("1.0 KB")
    expect(formatBytes(1536)).toBe("1.5 KB")
    expect(formatBytes(20 * 1024 * 1024)).toBe("20 MB")
    expect(formatBytes(3 * 1024 ** 3)).toBe("3.0 GB")
  })
})

describe("formatSpeed", () => {
  it("appends /s and falls back to a dash", () => {
    expect(formatSpeed(1024)).toBe("1.0 KB/s")
    expect(formatSpeed(0)).toBe("—")
  })
})

describe("formatEta", () => {
  it("formats minutes and seconds", () => {
    expect(formatEta(12)).toBe("12s")
    expect(formatEta(252)).toBe("4m 12s")
  })

  it("returns a dash for unusable input", () => {
    expect(formatEta(0)).toBe("—")
    expect(formatEta(-3)).toBe("—")
    expect(formatEta(Number.POSITIVE_INFINITY)).toBe("—")
  })
})

describe("formatClock", () => {
  it("pads seconds and keeps hours compact", () => {
    expect(formatClock(0)).toBe("0:00")
    expect(formatClock(5.9)).toBe("0:05")
    expect(formatClock(252)).toBe("4:12")
    expect(formatClock(3852)).toBe("1:04:12")
  })

  it("clamps invalid input to zero", () => {
    expect(formatClock(-10)).toBe("0:00")
    expect(formatClock(Number.NaN)).toBe("0:00")
  })
})

describe("backgroundImageUrl", () => {
  it("returns undefined without a value", () => {
    expect(backgroundImageUrl(null)).toBeUndefined()
    expect(backgroundImageUrl("")).toBeUndefined()
  })

  it("escapes quotes and backslashes so the url() stays valid", () => {
    expect(backgroundImageUrl("C:\\posters\\a.png")).toBe('url("C:\\\\posters\\\\a.png")')
    expect(backgroundImageUrl('a"b.png')).toBe('url("a\\"b.png")')
  })
})
