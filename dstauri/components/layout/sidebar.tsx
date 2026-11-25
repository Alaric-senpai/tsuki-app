"use client"

import Link from "next/link"
import { usePathname } from "next/navigation"
import {
  CompassIcon,
  DownloadIcon,
  HomeIcon,
  InfoIcon,
  LibraryIcon,
  MoonIcon,
  Settings2Icon,
} from "lucide-react"

import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from "@/components/ui/sidebar"

const PRIMARY_NAV = [
  { title: "Home", href: "/", icon: HomeIcon },
  { title: "Explore", href: "/explore", icon: CompassIcon },
  { title: "Library", href: "/library", icon: LibraryIcon },
  { title: "Downloads", href: "/downloads", icon: DownloadIcon },
  { title: "Settings", href: "/settings", icon: Settings2Icon },
]

export function TsukiSidebar() {
  const pathname = usePathname()

  const isActive = (href: string) =>
    href === "/" ? pathname === "/" : pathname.startsWith(href)

  return (
    <Sidebar collapsible="offcanvas" className="border-r border-border/60">
      <SidebarHeader className="px-3 py-4">
        <Link
          href="/"
          className="flex items-center gap-3 rounded-xl px-2 py-1.5 transition-colors hover:bg-sidebar-accent"
        >
          <span className="relative flex size-9 shrink-0 items-center justify-center rounded-xl bg-primary/10 ring-1 ring-primary/25">
            <MoonIcon className="size-4 text-primary" />
          </span>
          <span className="flex min-w-0 flex-col leading-tight">
            <span className="font-heading text-base font-extrabold tracking-tight text-foreground">
              Tsuki
            </span>
            <span className="telemetry text-[0.55rem] text-muted-foreground">
              Streaming hub
            </span>
          </span>
        </Link>
      </SidebarHeader>

      <SidebarContent>
        <SidebarGroup>
          <SidebarGroupLabel className="telemetry px-3 text-[0.55rem] text-muted-foreground/80">
            Navigate
          </SidebarGroupLabel>
          <SidebarGroupContent>
            <SidebarMenu>
              {PRIMARY_NAV.map((item) => (
                <SidebarMenuItem key={item.title} className="my-1 h-10 ">
                  <SidebarMenuButton
                    className="h-full"
                    isActive={isActive(item.href)}
                    tooltip={item.title}
                    render={<Link href={item.href} />}
                  >
                    <item.icon />
                    <span>{item.title}</span>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              ))}
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
      </SidebarContent>

      <SidebarFooter className="border-t border-border/60 p-3">
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton
              isActive={isActive("/about")}
              tooltip="About Tsuki"
              render={<Link href="/about" />}
            >
              <InfoIcon />
              <span>About</span>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
        <p className="telemetry px-2 pt-2 text-[0.5rem] text-muted-foreground/60">
          v0.1.0 · local-first
        </p>
      </SidebarFooter>
    </Sidebar>
  )
}
