"use client"

import { useEffect, useRef, useState, type FormEvent } from "react"
import { usePathname, useRouter } from "next/navigation"
import { SearchIcon } from "lucide-react"

import { WindowControls } from "@/components/layout/window-controls"
import { Separator } from "@/components/ui/separator"
import { SidebarTrigger } from "@/components/ui/sidebar"

const TITLES: Record<string, string> = {
  "/": "Home",
  "/explore": "Explore",
  "/library": "Library",
  "/downloads": "Downloads",
  "/settings": "Settings",
  "/about": "About",
  "/series": "Series",
  "/show": "Source series",
  "/watch": "Player",
}

export function TopBar() {
  const pathname = usePathname()
  const router = useRouter()
  const inputRef = useRef<HTMLInputElement>(null)
  const [term, setTerm] = useState("")
  const root = `/${pathname.split("/")[1] ?? ""}`
  const title = TITLES[root] ?? "Tsuki"

  useEffect(() => {
    const focusSearch = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault()
        inputRef.current?.focus()
      }
    }
    window.addEventListener("keydown", focusSearch)
    return () => window.removeEventListener("keydown", focusSearch)
  }, [])

  const searchLibrary = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    const query = term.trim()
    router.push(query ? `/library?q=${encodeURIComponent(query)}` : "/library")
    setTerm("")
    inputRef.current?.blur()
  }

  return (
    <header className="sticky top-0 z-20 flex h-(--header-height) shrink-0 items-center gap-3 border-b border-border/60 bg-background/80 px-4 backdrop-blur-xl lg:px-6">
      <SidebarTrigger className="-ml-1 text-muted-foreground hover:text-foreground" />
      <Separator orientation="vertical" className="h-4" />
      <span className="telemetry text-muted-foreground">{title}</span>

      <div className="ml-auto flex items-center gap-2">
        <form
          role="search"
          onSubmit={searchLibrary}
          className="hidden h-8 items-center gap-2 rounded-md border border-border/60 bg-surface-low px-3 text-xs text-muted-foreground transition-colors focus-within:border-primary/50 focus-within:text-foreground md:flex"
        >
          <SearchIcon className="size-3.5 shrink-0" aria-hidden="true" />
          <input
            ref={inputRef}
            type="search"
            value={term}
            onChange={(event) => setTerm(event.target.value)}
            placeholder="Search library…"
            aria-label="Search library"
            className="w-28 bg-transparent text-xs text-foreground outline-none placeholder:text-muted-foreground lg:w-44"
          />
          <kbd className="telemetry rounded border border-border/60 bg-surface-high px-1.5 py-0.5 text-[0.5rem]">
            ⌘K
          </kbd>
        </form>
        <span className="flex h-7 items-center gap-2 rounded-full border border-border/60 bg-surface-low px-3">
          <span className="size-1.5 rounded-full bg-emerald shadow-[0_0_6px_var(--emerald)]" />
          <span className="telemetry text-[0.5rem] text-muted-foreground">Local</span>
        </span>
        <WindowControls />
      </div>
    </header>
  )
}
