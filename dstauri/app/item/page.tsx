"use client"

import { Suspense } from "react"
import { useSearchParams } from "next/navigation"

import { LoadingGrid } from "@/components/common/data-state"
import { ItemDetails } from "@/components/features/explore/item-details"

function OnlineItemDetails() {
  const params = useSearchParams()
  return (
    <ItemDetails
      source={params.get("source") ?? "donghuastream"}
      id={params.get("id") ?? ""}
      url={params.get("url")}
      route="item"
    />
  )
}

export default function ItemPage() {
  return (
    <Suspense fallback={<LoadingGrid count={1} />}>
      <OnlineItemDetails />
    </Suspense>
  )
}
