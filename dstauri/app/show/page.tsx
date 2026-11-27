"use client"

import { Suspense } from "react"
import { useSearchParams } from "next/navigation"

import { LoadingGrid } from "@/components/common/data-state"
import { ItemDetails } from "@/components/features/explore/item-details"

function ShowDetails() {
  const params = useSearchParams()
  return (
    <ItemDetails
      source={params.get("source") ?? "donghuastream"}
      id={params.get("id") ?? ""}
      url={params.get("url")}
      route="show"
    />
  )
}

export default function ShowPage() {
  return (
    <Suspense fallback={<LoadingGrid count={1} />}>
      <ShowDetails />
    </Suspense>
  )
}
