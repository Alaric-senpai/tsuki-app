import Image from "next/image"
import type { CSSProperties, ReactNode } from "react"
import { PlayIcon } from "lucide-react"

import { backgroundImageUrl, cn } from "@/lib/utils"

function imageStyle(imageUrl: string | null | undefined): CSSProperties | undefined {
  const image = backgroundImageUrl(imageUrl)
  return image ? { backgroundImage: image } : undefined
}

export function PosterCard({
  title,
  meta,
  imageUrl,
  badges,
  quickAction,
  className,
}: {
  title: string
  meta?: string
  imageUrl?: string | null
  badges?: ReactNode
  quickAction?: ReactNode
  className?: string
}) {
  return (
    <article
      className={cn(
        "group relative overflow-hidden rounded-lg border border-border/50 bg-surface-low",
        className,
      )}
    >
      <div
        aria-hidden
        style={imageStyle(imageUrl)}
        className="aspect-[2/3] w-full bg-[radial-gradient(circle_at_30%_15%,var(--surface-highest),var(--surface-lowest)_72%)] bg-cover bg-center transition-transform duration-500 group-hover:scale-[1.03]"
      >
        {!imageUrl && (
          <Image
            src="/assets/placeholder-thumbail.jpeg"
            alt=""
            fill
            unoptimized
            sizes="(max-width:640px) 50vw,25vw"
            className="object-cover opacity-60"
            loading="lazy"
            aria-hidden
          />
        )}
      </div>
      <div className="absolute inset-0 bg-gradient-to-t from-surface-lowest via-surface-lowest/25 to-transparent" />
      {badges ? (
        <div className="absolute inset-x-0 top-0 flex flex-wrap gap-1.5 p-2">{badges}</div>
      ) : null}
      <div className="absolute inset-x-0 bottom-0 space-y-0.5 p-3">
        <p className="line-clamp-2 font-heading text-sm font-semibold leading-snug text-foreground">
          {title}
        </p>
        {meta ? (
          <p className="telemetry text-[0.5rem] text-muted-foreground">{meta}</p>
        ) : null}
      </div>
      {quickAction !== null ? (
        <span className="absolute right-2 top-2 flex size-8 items-center justify-center rounded-full bg-background/70 opacity-0 backdrop-blur transition-opacity group-hover:opacity-100 group-focus-within:opacity-100">
          {quickAction === undefined ? <PlayIcon className="size-3.5 text-primary" /> : quickAction}
        </span>
      ) : null}
    </article>
  )
}
