"use client"

import type { CSSProperties, ReactNode } from "react"

import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar"
import { TsukiSidebar } from "@/components/layout/sidebar"
import { TopBar } from "@/components/layout/top-bar"

export function AppShell({ children }: { children: ReactNode }) {
  return (
    <SidebarProvider
      style={
        {
          "--sidebar-width": "15rem",
          "--header-height": "3.25rem",
        } as CSSProperties
      }
    >
      <TsukiSidebar />
      <SidebarInset className="min-w-0 bg-background">
        <TopBar />
        <div className="flex flex-1 flex-col">{children}</div>
      </SidebarInset>
    </SidebarProvider>
  )
}
