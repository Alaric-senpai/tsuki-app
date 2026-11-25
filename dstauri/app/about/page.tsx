"use client"

import { useEffect, useState } from "react"
import { ExternalLinkIcon } from "lucide-react"

import { PageHeader } from "@/components/common/page-header"
import { Button } from "@/components/ui/button"
import { getVersion } from "@tauri-apps/api/app"

const SECTIONS = [
  {
    title: "A personal passion project",
    body: "Donghua Stream — now Tsuki — is by no means marketable. It is not sold, licensed, marketed or offered as a commercial product or service.",
  },
  {
    title: "No association",
    body: "Use of this software is not associated with, endorsed by, sponsored by or supported by the developer. The developer is not responsible for how third parties use, modify or distribute the code.",
  },
  {
    title: "No warranty",
    body: "The software is provided “as is”, without warranty of any kind, express or implied, including but not limited to the warranties of merchantability, fitness for a particular purpose and non-infringement.",
  },
  {
    title: "No liability",
    body: "In no event shall the developer be liable for any claim, damages or other liability arising from, out of, or in connection with the software or its use.",
  },
  {
    title: "No affiliation",
    body: "This project is not affiliated with YouTube, Google, or any content owner or studio. All trademarks, channel names and media belong to their respective owners.",
  },
  {
    title: "User responsibility",
    body: "You are responsible for complying with the terms of service of any site you access and with the copyright laws that apply where you live. Only download content you have the right to access and keep.",
  },
]

const REPOSITORY = "https://github.com/Alaric-senpai/tsuki-app"
const DEVELOPER = "Alaric-senpai"
const PRIORITIES = [
  "Derive AnimeCube's current playback endpoint so remote episodes resolve end-to-end.",
  "Build the 3-step Add Series wizard once its Stitch screen is published.",
  "Verify signed release packaging on Linux (.deb, .rpm, AppImage) and Windows/macOS.",
]

function InfoCard({ title, rows }: { title: string; rows: [string, string][] }) {
  return (
    <section className="rounded-xl border border-border/60 bg-surface-low p-5">
      <p className="telemetry text-[0.5rem] text-muted-foreground">{title}</p>
      <dl className="mt-3 space-y-2">
        {rows.map(([label, value]) => (
          <div key={label} className="flex flex-wrap justify-between gap-2 text-sm">
            <dt className="text-muted-foreground">{label}</dt>
            <dd className="font-mono text-xs text-foreground">{value}</dd>
          </div>
        ))}
      </dl>
    </section>
  )
}

export default function AboutPage() {
  const [version, setVersion] = useState("0.1.0")

  useEffect(() => {
    void getVersion()
      .then((value) => queueMicrotask(() => setVersion(value)))
      .catch(() => {})
  }, [])

  return (
    <>
      <PageHeader
        eyebrow="About"
        title="Tsuki"
        description="A cinematic hub for streaming and downloading the shows you follow."
      />

      <div className="mx-auto max-w-5xl space-y-4 px-6 py-6">
        <section className="rounded-xl border border-border/60 bg-surface-low p-5">
          <p className="telemetry text-[0.5rem] text-muted-foreground">Disclaimer</p>
          <p className="mt-2 text-sm leading-relaxed text-muted-foreground">
            Tsuki is a local-first desktop application. It ships no content and hosts no
            media — it only organises and plays what you point it at.
          </p>
        </section>

        <div className="grid gap-5 md:grid-cols-3">
          <InfoCard
            title="App"
            rows={[
              ["Name", "Tsuki"],
              ["Version", version],
              ["Mode", "Local-first desktop"],
              ["License", "Personal, non-commercial"],
            ]}
          />
          <InfoCard
            title="Runtime"
            rows={[
              ["Shell", "Tauri v2"],
              ["Backend", "Rust / SQLite"],
              ["Frontend", "Next.js · React · TypeScript"],
              ["UI", "shadcn/ui · Tailwind"],
              ["Bundled tools", "yt-dlp · ffmpeg"],
            ]}
          />
          <InfoCard
            title="Developer"
            rows={[
              ["Maintainer", DEVELOPER],
              ["Repository", "Alaric-senpai/tsuki-app"],
              ["Releases", "GitHub Releases"],
              ["Support", "Issues on GitHub"],
            ]}
          />
        </div>

        <section className="rounded-xl border border-border/60 bg-surface-low p-5">
          <p className="telemetry text-[0.5rem] text-muted-foreground">Developer section</p>
          <h2 className="mt-1 font-heading text-base font-semibold text-foreground">
            Project notes &amp; priorities
          </h2>
          <p className="mt-2 text-sm leading-relaxed text-muted-foreground">
            Tsuki is the Tauri successor to the PyQt6 Donghua Stream app. All catalog data
            lives in an in-app SQLite database, all network and process work runs in Rust,
            and the frontend is a static Next.js SPA. Third-party licences are recorded in
            THIRD_PARTY.md. Linux packages are published as AppImage, .deb and .rpm (Fedora).
          </p>
          <ul className="mt-3 list-disc space-y-1 pl-5 text-sm text-muted-foreground">
            {PRIORITIES.map((item) => (
              <li key={item}>{item}</li>
            ))}
          </ul>
          <Button variant="outline" size="sm" className="mt-4" render={<a href={REPOSITORY} target="_blank" rel="noreferrer" />} nativeButton={false}>
            View repository
            <ExternalLinkIcon />
          </Button>
        </section>

        <div className="grid gap-5 md:grid-cols-2 lg:grid-cols-3">
          {SECTIONS.map((section) => (
            <section
              key={section.title}
              className="rounded-xl border border-border/60 bg-surface-low p-5"
            >
              <h2 className="font-heading text-base font-semibold tracking-tight text-foreground">
                {section.title}
              </h2>
              <p className="mt-2 text-sm leading-relaxed text-muted-foreground">{section.body}</p>
            </section>
          ))}
        </div>

        <p className="telemetry text-[0.5rem] text-muted-foreground">Tsuki v{version}</p>
      </div>
    </>
  )
}
