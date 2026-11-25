"use client"

import { create } from "zustand"

import type { PlayableSource } from "@/lib/player/engine"

export interface MiniPlayerState {
  source: PlayableSource | null
  title: string
  subtitle: string
  open: (source: PlayableSource, title?: string, subtitle?: string) => void
  close: () => void
}

export const useMiniPlayer = create<MiniPlayerState>((set) => ({
  source: null,
  title: "",
  subtitle: "",
  open: (source, title = "", subtitle = "") =>
    set({ source, title, subtitle }),
  close: () => set({ source: null, title: "", subtitle: "" }),
}))
