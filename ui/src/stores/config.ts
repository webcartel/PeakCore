import { defineStore } from 'pinia'

export type ThemePreference = 'system' | 'dark' | 'light'
export type Language = 'ru' | 'en'
export type TrayMode = 'cpu_ram' | 'cpu' | 'ram'
export type AlertMetric = 'cpu' | 'ram'

export interface AlertRule {
  metric: AlertMetric
  threshold_pct: number
  duration_s: number
  cooldown_min: number
}

export interface Config {
  sampling: { tray_interval_ms: number; window_interval_ms: number }
  tray: { mode: TrayMode; warn_pct: number; crit_pct: number }
  history: { minutes_detail: number; hours_aggregate: number }
  ui: { theme: ThemePreference; language: Language }
  alerts: AlertRule[]
}

export interface AlertStatus {
  metric: AlertMetric
  threshold_pct: number
  duration_s: number
  cooldown_min: number
  value_pct: number
  triggered: boolean
  above_since_ms: number | null
  last_fired_ms: number | null
}

export interface AlertEvent {
  ts_ms: number
  metric: AlertMetric
  threshold_pct: number
  value_pct: number
}

export interface AlertsPayload {
  statuses: AlertStatus[]
  history: AlertEvent[]
}

function tauriAvailable() {
  return '__TAURI_INTERNALS__' in window
}

/// Older daemons may omit empty sections; the pages index into them directly.
function withDefaults(config: Config): Config {
  return {
    ...config,
    sampling: { ...config.sampling },
    tray: { ...config.tray },
    history: { ...config.history },
    ui: { ...config.ui },
    alerts: config.alerts ?? [],
  }
}

export const useConfigStore = defineStore('config', {
  state: () => ({
    config: null as Config | null,
    path: '',
    saving: false,
    error: null as string | null,
    loaded: false,
  }),
  actions: {
    async load() {
      if (!tauriAvailable()) return
      try {
        const { invoke } = await import('@tauri-apps/api/core')
        const payload = await invoke<{ config: Config; path: string }>('get_config')
        this.config = withDefaults(payload.config)
        this.path = payload.path
        this.error = null
      } catch (error) {
        this.error = String(error)
      } finally {
        this.loaded = true
      }
    },
    async save(next: Config): Promise<boolean> {
      this.saving = true
      try {
        const { invoke } = await import('@tauri-apps/api/core')
        const saved = await invoke<Config>('save_config', { config: withDefaults(next) })
        this.config = withDefaults(saved)
        this.error = null
        return true
      } catch (error) {
        this.error = String(error)
        return false
      } finally {
        this.saving = false
      }
    },
    async alerts(): Promise<AlertsPayload | null> {
      if (!tauriAvailable()) return null
      try {
        const { invoke } = await import('@tauri-apps/api/core')
        return await invoke<AlertsPayload>('get_alerts')
      } catch (error) {
        this.error = String(error)
        return null
      }
    },
  },
})
