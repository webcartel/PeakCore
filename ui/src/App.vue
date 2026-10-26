<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'
import { useRoute } from 'vue-router'
import {
  Activity, Bell, ChevronDown, Cpu, HardDrive, LayoutDashboard,
  MemoryStick, Maximize2, Minus, Minimize2, Network, Settings2, X,
} from '@lucide/vue'
import { useMetricsStore, type DashboardUpdate } from './stores/metrics'

const route = useRoute()
const metrics = useMetricsStore()
const theme = ref<'dark' | 'light'>('dark')
const maximized = ref(false)
let unlistenUpdates: (() => void) | undefined
let unlistenResize: (() => void) | undefined
let samplingPoll: number | undefined
let syncSamplingHandler: (() => void) | undefined
let sampling = false
let checkingVisibility = false
const navItems = [
  { label: 'Обзор', path: '/', icon: LayoutDashboard },
  { label: 'Процессы', path: '/processes', icon: Activity },
  { label: 'CPU', path: '/cpu', icon: Cpu },
  { label: 'Память', path: '/memory', icon: MemoryStick },
  { label: 'Диск', path: '/disk', icon: HardDrive },
  { label: 'Сеть', path: '/network', icon: Network },
  { label: 'Алерты', path: '/alerts', icon: Bell },
]

function toggleTheme() {
  theme.value = theme.value === 'dark' ? 'light' : 'dark'
  document.documentElement.dataset.theme = theme.value
}

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
      <span class="current-section">{{ route.meta.title ?? 'Система' }}</span>
      <div class="title-spacer" data-tauri-drag-region></div>
      <button class="icon-button theme-button" title="Переключить тему" @click="toggleTheme">◐</button>
      <button class="window-button" title="Свернуть" @click="windowAction('minimize')"><Minus :size="14" /></button>
      <button class="window-button" :title="maximized ? 'Восстановить размер' : 'Развернуть на весь экран'" @click="toggleWindowSize"><component :is="maximized ? Minimize2 : Maximize2" :size="14" /></button>
      <button class="window-button close-button" title="Закрыть" @click="windowAction('close')"><X :size="15" /></button>
    </header>
    <div class="workspace">
      <aside class="sidebar">
        <div class="nav-caption">МОНИТОРИНГ</div>
        <nav>
          <RouterLink v-for="item in navItems" :key="item.path" :to="item.path" class="nav-item">
            <component :is="item.icon" :size="17" :stroke-width="1.8" />
            <span>{{ item.label }}</span>
            <span v-if="item.path === '/'" class="nav-active-dot"></span>
          </RouterLink>
        </nav>
        <div class="sidebar-bottom">
          <RouterLink to="/settings" class="nav-item"><Settings2 :size="17" /><span>Настройки</span></RouterLink>
          <div class="daemon-state"><span class="online-dot" :class="{ offline: !metrics.daemonConnected }"></span><span>{{ metrics.daemonConnected ? 'Демон активен' : 'Без связи с демоном' }}</span><ChevronDown :size="13" /></div>
          <div class="version">PEAKCORE <span>0.1.0</span></div>
        </div>
      </aside>
      <main class="main-content"><RouterView /></main>
    </div>
  </div>
</template>
