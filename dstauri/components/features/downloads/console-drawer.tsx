"use client"

import { TerminalIcon } from "lucide-react"

import { ConsoleView } from "@/components/common/console-view"
import { Button } from "@/components/ui/button"
import {
  Drawer,
  DrawerContent,
  DrawerDescription,
  DrawerHeader,
  DrawerTitle,
  DrawerTrigger,
} from "@/components/ui/drawer"

/** Session-wide yt-dlp output in a drawer, with a one-click copy. */
export function ConsoleDrawer({ lines }: { lines: string[] }) {
  return (
    <Drawer>
      <DrawerTrigger render={<Button variant="outline" size="sm" />}>
        <TerminalIcon />
        Console{lines.length > 0 ? ` (${lines.length})` : ""}
      </DrawerTrigger>
      <DrawerContent className="max-h-[80dvh]">
        <DrawerHeader>
          <DrawerTitle>Download console</DrawerTitle>
          <DrawerDescription>
            Output from this session&apos;s downloads. Copy a log to paste it
            into a bug report.
          </DrawerDescription>
        </DrawerHeader>
        <div className="min-h-0 flex-1 overflow-auto px-4 pb-4">
          <ConsoleView
            lines={lines}
            emptyLabel="No console output in this session yet."
          />
        </div>
      </DrawerContent>
    </Drawer>
  )
}
