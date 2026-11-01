<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import {
  Activity, Bell, ChevronDown, Cpu, HardDrive, LayoutDashboard,
  MemoryStick, Maximize2, Minus, Minimize2, Network, Settings2, X, Gpu,
} from '@lucide/vue'
import { useMetricsStore, type DashboardUpdate } from './stores/metrics'
import { useConfigStore } from './stores/config'
import { setLocale, t, type MessageKey } from './i18n'

const route = useRoute()
const metrics = useMetricsStore()
const config = useConfigStore()
const resolvedTheme = ref<'dark' | 'light'>('dark')
const maximized = ref(false)
let unlistenUpdates: (() => void) | undefined
let unlistenResize: (() => void) | undefined
let samplingPoll: number | undefined
let syncSamplingHandler: (() => void) | undefined
let sampling = false
let checkingVisibility = false
let systemTheme: MediaQueryList | undefined
const navItems: { key: MessageKey; path: string; icon: typeof LayoutDashboard }[] = [
  { key: 'nav.overview', path: '/', icon: LayoutDashboard },
  { key: 'nav.processes', path: '/processes', icon: Activity },
  { key: 'nav.cpu', path: '/cpu', icon: Cpu },
  { key: 'nav.memory', path: '/memory', icon: MemoryStick },
  { key: 'nav.disk', path: '/disk', icon: HardDrive },
  { key: 'nav.network', path: '/network', icon: Network },
  { key: 'nav.gpu', path: '/gpu', icon: Gpu },
  { key: 'nav.alerts', path: '/alerts', icon: Bell },
]

const sectionTitle = () => {
  const key = route.meta.titleKey as MessageKey | undefined
  return key ? t(key) : 'PeakCore'
}

function resolveTheme(): 'dark' | 'light' {
  const preference = config.config?.ui.theme ?? 'system'
  if (preference === 'system') return systemTheme?.matches ? 'light' : 'dark'
  return preference
}

function applyTheme() {
  resolvedTheme.value = resolveTheme()
  document.documentElement.dataset.theme = resolvedTheme.value
}

async function toggleTheme() {
  const next = resolvedTheme.value === 'dark' ? 'light' : 'dark'
  resolvedTheme.value = next
  document.documentElement.dataset.theme = next
  if (config.config) {
    await config.save({ ...config.config, ui: { ...config.config.ui, theme: next } })
  }
}

watch(() => config.config?.ui.theme, applyTheme)
watch(
  () => config.config?.ui.language,
  (language) => setLocale(language ?? 'ru'),
)

let appliedConfigRevision: number | null = null
watch(
  () => metrics.snapshot?.config_revision,
  async (revision) => {
    if (revision === undefined) return
    if (appliedConfigRevision === null || revision === appliedConfigRevision) {
      appliedConfigRevision = revision
      return
    }
    // The tray menu or another client changed the settings: refresh locally.
    appliedConfigRevision = revision
    await config.load()
    applyTheme()
    setLocale(config.config?.ui.language ?? 'ru')
  },
)

async function windowAction(action: 'minimize' | 'close') {
  if (!('__TAURI_INTERNALS__' in window)) return
  if (action === 'close') await stopSampling()
  const { getCurrentWindow } = await import('@tauri-apps/api/window')
  const current = getCurrentWindow()
  if (action === 'minimize') await current.minimize()
  else await current.close()
}

async function toggleWindowSize() {
  if (!('__TAURI_INTERNALS__' in window)) return
  const { getCurrentWindow } = await import('@tauri-apps/api/window')
  const current = getCurrentWindow()
  if (await current.isMaximized()) await current.unmaximize()
  else await current.maximize()
  maximized.value = await current.isMaximized()
}

function handleTitlebarDoubleClick(event: MouseEvent) {
  if (event.target instanceof Element && event.target.closest('button')) return
  void toggleWindowSize()
}

async function stopSampling() {
  if (!sampling) return
  const { invoke } = await import('@tauri-apps/api/core')
  try {
    await invoke('stop_sampling')
  } catch (error) {
    console.error('Could not stop PeakCore sampling cleanly', error)
  } finally {
    sampling = false
  }
}

onMounted(async () => {
  systemTheme = window.matchMedia('(prefers-color-scheme: light)')
  systemTheme.addEventListener('change', () => {
    if ((config.config?.ui.theme ?? 'system') === 'system') applyTheme()
  })
  if (!('__TAURI_INTERNALS__' in window)) return
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window')
    const current = getCurrentWindow()
    // Show immediately after the Vue root mounts. Keep window visibility
    // independent of IPC and sampling setup so those failures cannot hide UI.
    await current.show()
    maximized.value = await current.isMaximized()
    unlistenResize = await current.onResized(async () => {
      maximized.value = await current.isMaximized()
    })
    const [{ listen }, { invoke }] = await Promise.all([
      import('@tauri-apps/api/event'),
      import('@tauri-apps/api/core'),
    ])
    unlistenUpdates = await listen<DashboardUpdate>('dashboard-update', (event) => {
      metrics.apply(event.payload)
    })
    await config.load()
    applyTheme()
    setLocale(config.config?.ui.language ?? 'ru')
    if (config.config) {
      await invoke('set_sampling_interval', {
        intervalMs: config.config.sampling.window_interval_ms,
      })
    }
    const syncSampling = async () => {
      if (checkingVisibility) return
      checkingVisibility = true
      try {
        const visible = document.visibilityState === 'visible' && !(await current.isMinimized())
        if (visible && !sampling) {
          await invoke('start_sampling')
          sampling = true
        } else if (!visible && sampling) {
          await invoke('stop_sampling')
          sampling = false
        }
      } finally {
        checkingVisibility = false
      }
    }
    syncSamplingHandler = syncSampling
    document.addEventListener('visibilitychange', syncSamplingHandler)
    samplingPoll = window.setInterval(syncSampling, 750)
    await syncSampling()
  } catch (error) {
    console.error('Could not show PeakCore window', error)
  }
})

onUnmounted(() => {
  if (samplingPoll !== undefined) window.clearInterval(samplingPoll)
  if (syncSamplingHandler) document.removeEventListener('visibilitychange', syncSamplingHandler)
  unlistenUpdates?.()
  unlistenResize?.()
  void stopSampling()
})
</script>

<template>
  <div class="app-shell">
    <header class="titlebar" data-tauri-drag-region @dblclick="handleTitlebarDoubleClick">
      <div class="brand-mark"><Activity :size="15" /></div>
      <span class="brand-name">Peak<span>Core</span></span>
      <span class="title-divider"></span>
      <span class="current-section">{{ sectionTitle() }}</span>
      <div class="title-spacer" data-tauri-drag-region></div>
      <button class="icon-button theme-button" :title="t('app.toggleTheme')" @click="toggleTheme">◐</button>
      <button class="window-button" :title="t('app.minimize')" @click="windowAction('minimize')"><Minus :size="14" /></button>
      <button class="window-button" :title="maximized ? t('app.restore') : t('app.maximize')" @click="toggleWindowSize"><component :is="maximized ? Minimize2 : Maximize2" :size="14" /></button>
      <button class="window-button close-button" :title="t('app.close')" @click="windowAction('close')"><X :size="15" /></button>
    </header>
    <div class="workspace">
      <aside class="sidebar">
        <div class="nav-caption">{{ t('app.monitoring') }}</div>
        <nav>
          <RouterLink v-for="item in navItems" :key="item.path" :to="item.path" class="nav-item">
            <component :is="item.icon" :size="17" :stroke-width="1.8" />
            <span>{{ t(item.key) }}</span>
            <span v-if="item.path === '/'" class="nav-active-dot"></span>
          </RouterLink>
        </nav>
        <div class="sidebar-bottom">
          <RouterLink to="/settings" class="nav-item"><Settings2 :size="17" /><span>{{ t('nav.settings') }}</span></RouterLink>
          <div class="daemon-state"><span class="online-dot" :class="{ offline: !metrics.daemonConnected }"></span><span>{{ metrics.daemonConnected ? t('app.daemonOnline') : t('app.daemonOffline') }}</span><ChevronDown :size="13" /></div>
          <div class="version">PEAKCORE <span>0.1.0</span></div>
        </div>
      </aside>
      <main class="main-content"><RouterView /></main>
    </div>
  </div>
</template>
