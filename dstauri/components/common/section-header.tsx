import Link from "next/link"
import { ArrowRightIcon } from "lucide-react"

export function SectionHeader({
  title,
  eyebrow,
  href,
  actionLabel = "View all",
}: {
  title: string
  eyebrow?: string
  href?: string
  actionLabel?: string
}) {
  return (
    <div className="mb-4 flex items-end justify-between gap-4">
      <div className="min-w-0">
        {eyebrow ? (
          <p className="telemetry text-[0.55rem] text-muted-foreground">{eyebrow}</p>
        ) : null}
        <h2 className="font-heading text-lg font-semibold tracking-tight text-foreground">
          {title}
        </h2>
      </div>
      {href ? (
        <Link
          href={href}
          className="telemetry flex shrink-0 items-center gap-1 text-primary transition-opacity hover:opacity-80"
        >
          {actionLabel}
          <ArrowRightIcon className="size-3" />
        </Link>
      ) : null}
    </div>
  )
}
