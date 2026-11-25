import { describe, expect, it } from "vitest"

import { toAppIpcError } from "@/lib/ipc/client"

describe("toAppIpcError", () => {
  it("keeps the Rust { code, message, detail } shape", () => {
    const error = { code: "db", message: "Could not read the library.", detail: "no such table" }
    expect(toAppIpcError(error)).toEqual({
      code: "db",
      message: "Could not read the library.",
      detail: "no such table",
    })
  })

  it("defaults the code and message for partial payloads", () => {
    expect(toAppIpcError({})).toEqual({
      code: "unknown",
      message: "Tsuki could not complete that request.",
      detail: undefined,
    })
  })

  it("stringifies a non-string detail instead of dropping it", () => {
    const result = toAppIpcError({ code: "io", message: "Failed.", detail: 42 })
    expect(result.detail).toBe("42")
  })

  it("reads the message from a plain Error but keeps no detail", () => {
    const result = toAppIpcError(new Error("boom"))
    expect(result.code).toBe("unknown")
    expect(result.message).toBe("boom")
    expect(result.detail).toBeUndefined()
  })

  it("never throws on primitives", () => {
    expect(toAppIpcError("nope").message).toBe("Tsuki could not complete that request.")
    expect(toAppIpcError(null).code).toBe("unknown")
  })
})
