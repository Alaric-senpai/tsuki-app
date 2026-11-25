"use client"

import { PageHeader } from "@/components/common/page-header"
import { StatusChip } from "@/components/common/status-chip"
import { SettingsPanel } from "@/components/features/settings/settings-panel"

export default function SettingsPage() {
  return (
    <>
      <PageHeader
        eyebrow="Preferences"
        title="Settings"
        description="Settings are saved to a versioned JSON file in your OS config directory."
        actions={<StatusChip tone="active" label="Local JSON config" />}
      />
      <SettingsPanel />
    </>
  )
}
