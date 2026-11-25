import type { Metadata } from "next"
import localFont from "next/font/local"

import { ExternalLinkGuard } from "@/components/external-link-guard"
import { MiniPlayer } from "@/components/player/mini-player"
import { ThemeProvider } from "@/components/theme-provider"
import { TooltipProvider } from "@/components/ui/tooltip"
import { Toaster } from "@/components/ui/sonner"
import { cn } from "@/lib/utils"
import { AppShell } from "@/components/layout/app-shell"

import "./globals.css"

// Fonts are bundled in public/fonts so the app never needs a network request
// to Google Fonts (offline installs and the Tauri webview both benefit).
const geist = localFont({
  src: "../public/fonts/geist-latin.woff2",
  variable: "--font-geist-sans",
  weight: "100 900",
  display: "swap",
})

const jakarta = localFont({
  src: "../public/fonts/plus-jakarta-sans-latin.woff2",
  variable: "--font-jakarta",
  weight: "200 800",
  display: "swap",
})

const jetbrains = localFont({
  src: "../public/fonts/jetbrains-mono-latin.woff2",
  variable: "--font-jetbrains",
  weight: "100 800",
  display: "swap",
})

export const metadata: Metadata = {
  title: {
    default: "Tsuki",
    template: "%s · Tsuki",
  },
  description:
    "A cinematic hub for streaming and downloading the shows you follow.",
}

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode
}>) {
  return (
    <html
      lang="en"
      suppressHydrationWarning
      className={cn(
        "antialiased",
        geist.variable,
        jakarta.variable,
        jetbrains.variable
      )}
    >
      <body>
        <ThemeProvider defaultTheme="dark" enableSystem={false}>
          <AppShell>
          <TooltipProvider delay={200}>
            <div data-ui-scroll-container className="mb-12">
              <ExternalLinkGuard />
              {children}
            </div>
            <MiniPlayer />
            <Toaster position="bottom-right" richColors closeButton />
          </TooltipProvider>
          </AppShell>
        </ThemeProvider>
      </body>
    </html>
  )
}
