import { PageHeader } from "@/components/common/page-header"

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

export default function AboutPage() {
  return (
    <>
      <PageHeader
        eyebrow="About"
        title="Tsuki"
        description="A cinematic hub for streaming and downloading the shows you follow."
      />

      <div className="max-w-5xl mx-auto space-y-4 px-6 py-6">
        <section className="rounded-xl border border-border/60 bg-surface-low p-5">
          <p className="telemetry text-[0.5rem] text-muted-foreground">Disclaimer</p>
          <p className="mt-2 text-sm leading-relaxed text-muted-foreground">
            Tsuki is a local-first desktop application. It ships no content and hosts no
            media — it only organises and plays what you point it at.
          </p>
        </section>
        <div className="grid grid-cols-3 gap-5">
          {SECTIONS.map((section) => (
            <section
              key={section.title}
              className="rounded-xl border border-border/60 bg-surface-low p-5"
            >
              <h2 className="font-heading text-base font-semibold tracking-tight text-foreground">
                {section.title}
              </h2>
              <p className="mt-2 text-sm leading-relaxed text-muted-foreground">
                {section.body}
              </p>
            </section>
          ))}
        </div>

        <p className="telemetry text-[0.5rem] text-muted-foreground">
          Tsuki v0.1.0 
        </p>
      </div>
    </>
  )
}
