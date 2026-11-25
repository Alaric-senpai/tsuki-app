import type { Metadata } from "next"
import { Geist, JetBrains_Mono, Plus_Jakarta_Sans } from "next/font/google"

import { ExternalLinkGuard } from "@/components/external-link-guard"
import { ThemeProvider } from "@/components/theme-provider"
import { TooltipProvider } from "@/components/ui/tooltip"
import { Toaster } from "@/components/ui/sonner"
import { cn } from "@/lib/utils"
import { AppShell } from "@/components/layout/app-shell"

import "./globals.css"

const geist = Geist({
  variable: "--font-geist-sans",
  subsets: ["latin"],
})

const jakarta = Plus_Jakarta_Sans({
  variable: "--font-jakarta",
  subsets: ["latin"],
})

const jetbrains = JetBrains_Mono({
  variable: "--font-jetbrains",
  subsets: ["latin"],
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
            <Toaster position="bottom-right" richColors closeButton />
          </TooltipProvider>
          </AppShell>
        </ThemeProvider>
      </body>
    </html>
  )
}
