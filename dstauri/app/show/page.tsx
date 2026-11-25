import Link from "next/link"
import { ArrowLeftIcon, CompassIcon } from "lucide-react"

import { EmptyState } from "@/components/common/empty-state"
import { PageHeader } from "@/components/common/page-header"
import { Button } from "@/components/ui/button"

export default function ShowPage() {
  return (
    <>
      <PageHeader
        eyebrow="Source catalog"
        title="Show detail"
        description="Catalog entry, synopsis, recommendations and the episode list from a remote source."
        actions={
          <Button variant="ghost" nativeButton={false} render={<Link href="/explore" />}>
            <ArrowLeftIcon />
            Back to explore
          </Button>
        }
      />
      <div className="px-6 py-6">
        <EmptyState
          icon={CompassIcon}
          title="Source scrapers land in Phase 3"
          description="AnimeCube and DonghuaStream resolution, plus the Add Series wizard, arrive with the sources milestone. This route is reserved for /show?source=&id=…"
        />
      </div>
    </>
  )
}
