import { ipcCall } from "./client"

export interface YouTubeConfig {
  /** Opt-in: the YouTube source is disabled until the user enables it. */
  enabled: boolean
  api_key: string
  region_code: string
  language: string
  safe_search: string
}

export interface ExploreConfig {
  result_limit: number
  cache_minutes: number
  /** Source the Home "Updates" section re-checks; never YouTube. */
  update_source: string
}

export interface PlayerConfig {
  engine: string
  volume: number
  external_player: string
  remember_position: boolean
  auto_play_next: boolean
}

/** Structured setter arguments are camelCase on the IPC boundary. */
export interface SetPlayerConfigArgs {
  engine: string
  volume: number
  externalPlayer: string
  rememberPosition: boolean
  autoPlayNext: boolean
}

export interface SetExploreConfigArgs {
  resultLimit: number
  cacheMinutes: number
  updateSource?: string
}

export interface SetYouTubeConfigArgs {
  enabled: boolean
  apiKey: string
  regionCode: string
  language: string
  safeSearch: string
}

export interface ConfigMeta {
  updated_at: string | null
  legacy_settings_imported: boolean
  imported_legacy_config_at: string | null
  legacy_data_imported: boolean
  legacy_data_imported_at: string | null
}

export interface AppConfig {
  version: number
  youtube: YouTubeConfig
  settings: Record<string, string>
  explore: ExploreConfig
  player: PlayerConfig
  meta: ConfigMeta
}

export const DEFAULT_CONFIG: AppConfig = {
  version: 3,
  youtube: {
    enabled: false,
    api_key: "",
    region_code: "US",
    language: "en",
    safe_search: "moderate",
  },
  settings: {
    global_save_dir: "~/Videos",
    default_quality: "720p",
    browser_cookies: "none",
    max_retries: "3",
    default_container: "mp4",
    sub_langs: "en",
    rate_limit_mb: "0",
    per_series_subfolder: "1",
    concurrent_downloads: "3",
    auto_check_app_updates: "1",
    auto_download_app_updates: "1",
  },
  explore: {
    result_limit: 18,
    cache_minutes: 20,
    update_source: "donghuastream",
  },
  player: {
    engine: "auto",
    volume: 1,
    external_player: "",
    remember_position: true,
    auto_play_next: true,
  },
  meta: {
    updated_at: null,
    legacy_settings_imported: false,
    imported_legacy_config_at: null,
    legacy_data_imported: false,
    legacy_data_imported_at: null,
  },
}

export function getConfig(): Promise<AppConfig> {
  return ipcCall<AppConfig>("get_config", DEFAULT_CONFIG)
}

export function setSetting(key: string, value: string): Promise<AppConfig> {
  return ipcCall<AppConfig>("set_setting", DEFAULT_CONFIG, { key, value })
}

export function setPlayerConfig(args: SetPlayerConfigArgs): Promise<AppConfig> {
  return ipcCall<AppConfig>("set_player_config", DEFAULT_CONFIG, {
    engine: args.engine,
    volume: args.volume,
    externalPlayer: args.externalPlayer,
    external_player: args.externalPlayer,
    rememberPosition: args.rememberPosition,
    remember_position: args.rememberPosition,
    autoPlayNext: args.autoPlayNext,
    auto_play_next: args.autoPlayNext,
  })
}

export function setExploreConfig(args: SetExploreConfigArgs): Promise<AppConfig> {
  return ipcCall<AppConfig>("set_explore_config", DEFAULT_CONFIG, {
    resultLimit: args.resultLimit,
    result_limit: args.resultLimit,
    cacheMinutes: args.cacheMinutes,
    cache_minutes: args.cacheMinutes,
    updateSource: args.updateSource ?? null,
  })
}

export function setYouTubeConfig(args: SetYouTubeConfigArgs): Promise<AppConfig> {
  return ipcCall<AppConfig>("set_youtube_config", DEFAULT_CONFIG, {
    enabled: args.enabled,
    apiKey: args.apiKey,
    api_key: args.apiKey,
    regionCode: args.regionCode,
    region_code: args.regionCode,
    language: args.language,
    safeSearch: args.safeSearch,
    safe_search: args.safeSearch,
  })
}

export function getConfigPath(): Promise<string> {
  return ipcCall<string>("get_config_path", "Browser preview")
}

/** Hand a local file to the configured external player (or the OS default). */
export function openInExternalPlayer(path: string): Promise<void> {
  return ipcCall<void>("open_in_external_player", undefined, { path })
}
