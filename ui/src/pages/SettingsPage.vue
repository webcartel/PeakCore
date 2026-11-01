<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Cpu, FolderCog, Moon, RefreshCw, Save, Sun, Monitor } from '@lucide/vue'
import { useConfigStore, type Config, type ThemePreference } from '../stores/config'
import { t, type MessageKey } from '../i18n'

const store = useConfigStore()
const draft = ref<Config | null>(null)
const error = ref('')
const saved = ref(false)
let savedTimer: number | undefined

watch(
  () => store.config,
  (value) => {
    if (value) draft.value = JSON.parse(JSON.stringify(value)) as Config
  },
  { immediate: true },
)

const themes: { value: ThemePreference; key: MessageKey; icon: typeof Moon }[] = [
  { value: 'system', key: 'settings.theme.system', icon: Monitor },
  { value: 'dark', key: 'settings.theme.dark', icon: Moon },
  { value: 'light', key: 'settings.theme.light', icon: Sun },
]

const dirty = computed(() => {
  if (!draft.value || !store.config) return false
  return JSON.stringify(draft.value) !== JSON.stringify(store.config)
})

function intervalOptions(current: number, options: number[]) {
  return options.includes(current) ? options : [...options, current].sort((a, b) => a - b)
}

function formatInterval(ms: number) {
  return ms >= 1000 ? `${ms / 1000} ${t('unit.sec')}` : `${ms} ms`
}

async function save() {
  if (!draft.value) return
  const next = draft.value
  if (next.tray.warn_pct >= next.tray.crit_pct) {
    error.value = t('settings.warnOrder')
    return
  }
  error.value = ''
  const ok = await store.save(next)
  if (!ok) {
    error.value = store.error ?? t('settings.saveFailed')
    return
  }
  const { invoke } = await import('@tauri-apps/api/core')
  await invoke('set_sampling_interval', { intervalMs: next.sampling.window_interval_ms })
  saved.value = true
  window.clearTimeout(savedTimer)
  savedTimer = window.setTimeout(() => (saved.value = false), 1800)
}

function reload() {
  error.value = ''
  void store.load()
}
</script>

<template>
  <main class="resource-page settings-page">
    <header class="resource-page-heading">
      <div>
        <div class="eyebrow">{{ t('eyebrow.settings') }}</div>
        <h1>{{ t('settings.title') }}</h1>
        <p>{{ t('settings.subtitle') }}</p>
      </div>
      <div class="settings-actions">
        <button class="ghost-button" type="button" @click="reload"><RefreshCw :size="14" /> {{ t('settings.refresh') }}</button>
        <button class="primary-button" type="button" :disabled="!draft || store.saving" @click="save">
          <Save :size="15" /> {{ store.saving ? t('settings.saving') : t('settings.save') }}
        </button>
      </div>
    </header>

    <p v-if="error" class="settings-banner error">{{ error }}</p>
    <p v-else-if="saved" class="settings-banner success">{{ t('settings.saved') }}</p>
    <p v-else-if="store.error" class="settings-banner error">{{ store.error }}</p>

    <div v-if="!draft" class="detail-empty">{{ t('settings.waitingDaemon') }}</div>

    <form v-else class="settings-grid" @submit.prevent="save">
      <section class="setting-card">
        <div class="setting-heading">
          <span class="detail-icon"><Cpu :size="17" /></span>
          <div><h2>{{ t('settings.sampling') }}</h2><p>{{ t('settings.samplingHint') }}</p></div>
        </div>
        <label class="setting-row">
          <span><b>{{ t('settings.trayInterval') }}</b><small>{{ t('settings.trayIntervalHint') }}</small></span>
          <select v-model.number="draft.sampling.tray_interval_ms" class="select-input">
            <option v-for="value in intervalOptions(draft.sampling.tray_interval_ms, [1000, 2000, 5000])" :key="value" :value="value">
              {{ formatInterval(value) }}
            </option>
          </select>
        </label>
        <label class="setting-row">
          <span><b>{{ t('settings.windowInterval') }}</b><small>{{ t('settings.windowIntervalHint') }}</small></span>
          <select v-model.number="draft.sampling.window_interval_ms" class="select-input">
            <option v-for="value in intervalOptions(draft.sampling.window_interval_ms, [500, 1000, 2000])" :key="value" :value="value">
              {{ formatInterval(value) }}
            </option>
          </select>
        </label>
      </section>

      <section class="setting-card">
        <div class="setting-heading">
          <span class="detail-icon"><Monitor :size="17" /></span>
          <div><h2>{{ t('settings.tray') }}</h2><p>{{ t('settings.trayHint') }}</p></div>
        </div>
        <label class="setting-row">
          <span><b>{{ t('settings.mode') }}</b><small>{{ t('settings.modeHint') }}</small></span>
          <select v-model="draft.tray.mode" class="select-input">
            <option value="cpu_ram">CPU + RAM</option>
            <option value="cpu">CPU</option>
            <option value="ram">RAM</option>
          </select>
        </label>
        <label class="setting-row">
          <span><b>{{ t('settings.warn') }}</b><small>{{ t('settings.warnHint') }}</small></span>
          <input v-model.number="draft.tray.warn_pct" class="number-input" type="number" min="1" max="99" />
        </label>
        <label class="setting-row">
          <span><b>{{ t('settings.crit') }}</b><small>{{ t('settings.critHint') }}</small></span>
          <input v-model.number="draft.tray.crit_pct" class="number-input" type="number" min="2" max="100" />
        </label>
      </section>

      <section class="setting-card">
        <div class="setting-heading">
          <span class="detail-icon"><Moon :size="17" /></span>
          <div><h2>{{ t('settings.interface') }}</h2><p>{{ t('settings.interfaceHint') }}</p></div>
        </div>
        <div class="setting-row">
          <span><b>{{ t('settings.theme') }}</b><small>{{ t('settings.themeHint') }}</small></span>
          <div class="segmented">
            <button
              v-for="theme in themes"
              :key="theme.value"
              type="button"
              :class="{ active: draft.ui.theme === theme.value }"
              @click="draft.ui.theme = theme.value"
            >
              <component :is="theme.icon" :size="14" /> {{ t(theme.key) }}
            </button>
          </div>
        </div>
        <label class="setting-row">
          <span><b>{{ t('settings.language') }}</b><small>{{ t('settings.languageHint') }}</small></span>
          <select v-model="draft.ui.language" class="select-input">
            <option value="ru">Русский</option>
            <option value="en">English</option>
          </select>
        </label>
      </section>

      <section class="setting-card">
        <div class="setting-heading">
          <span class="detail-icon"><FolderCog :size="17" /></span>
          <div><h2>{{ t('settings.configFile') }}</h2><p>{{ t('settings.configFileHint') }}</p></div>
        </div>
        <p class="setting-path">{{ store.path || t('settings.configPathMissing') }}</p>
        <p class="setting-note">
          {{ t('settings.historyNote', { minutes: draft.history.minutes_detail, hours: draft.history.hours_aggregate }) }}
        </p>
      </section>
    </form>
    <p v-if="dirty" class="settings-unsaved">{{ t('settings.unsaved') }}</p>
  </main>
</template>
