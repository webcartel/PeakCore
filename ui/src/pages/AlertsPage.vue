<script setup lang="ts">
import { computed, onMounted, onUnmounted, reactive, ref } from 'vue'
import { AlertTriangle, BellRing, Plus, Trash2 } from '@lucide/vue'
import {
  useConfigStore,
  type AlertMetric,
  type AlertRule,
  type AlertStatus,
  type AlertsPayload,
  type Config,
} from '../stores/config'
import { formatDuration, formatTime, t, tn } from '../i18n'

const store = useConfigStore()
const payload = ref<AlertsPayload | null>(null)
const error = ref('')
const adding = ref(false)
let poll: number | undefined

const form = reactive<AlertRule>({
  metric: 'ram',
  threshold_pct: 90,
  duration_s: 30,
  cooldown_min: 10,
})

const statuses = computed<AlertStatus[]>(() => {
  if (payload.value) return payload.value.statuses
  return (store.config?.alerts ?? []).map((rule) => ({
    ...rule,
    value_pct: 0,
    triggered: false,
    above_since_ms: null,
    last_fired_ms: null,
  }))
})

const history = computed(() => [...(payload.value?.history ?? [])].reverse().slice(0, 20))
const breaching = computed(() => statuses.value.some((item) => item.triggered))

function metricLabel(metric: AlertMetric) {
  return t(metric === 'cpu' ? 'metric.cpu' : 'metric.ram')
}

async function refresh() {
  payload.value = await store.alerts()
  if (store.error) error.value = store.error
}

async function addRule() {
  if (!store.config) return
  if (store.config.alerts.length >= 32) {
    error.value = t('alerts.maxRules')
    return
  }
  error.value = ''
  adding.value = true
  try {
    const next = JSON.parse(JSON.stringify(store.config)) as Config
    next.alerts.push({ ...form })
    const ok = await store.save(next)
    if (!ok) {
      error.value = store.error ?? t('alerts.ruleSaveFailed')
      return
    }
    await refresh()
  } catch (failure) {
    error.value = `${t('alerts.ruleSaveFailed')} ${String(failure)}`
  } finally {
    adding.value = false
  }
}

async function removeRule(index: number) {
  if (!store.config) return
  error.value = ''
  try {
    const next = JSON.parse(JSON.stringify(store.config)) as Config
    next.alerts.splice(index, 1)
    const ok = await store.save(next)
    if (!ok) {
      error.value = store.error ?? t('alerts.ruleDeleteFailed')
      return
    }
    await refresh()
  } catch (failure) {
    error.value = `${t('alerts.ruleDeleteFailed')} ${String(failure)}`
  }
}

onMounted(async () => {
  if (!store.loaded) await store.load()
  await refresh()
  poll = window.setInterval(refresh, 2_000)
})

onUnmounted(() => {
  if (poll !== undefined) window.clearInterval(poll)
})
</script>

<template>
  <main class="resource-page alerts-page">
    <header class="resource-page-heading">
      <div>
        <div class="eyebrow">{{ t('eyebrow.alerts') }}</div>
        <h1>{{ t('alerts.title') }}</h1>
        <p>{{ t('alerts.subtitle') }}</p>
      </div>
      <span class="resource-live-badge"><i></i>{{ breaching ? t('alerts.stateBreach') : t('alerts.stateOk') }}</span>
    </header>

    <p v-if="error" class="settings-banner error">{{ error }}</p>

    <section class="detail-table-card">
      <div class="detail-list-heading">
        <div><h2>{{ t('alerts.rules') }}</h2><p>{{ t('alerts.rulesHint') }}</p></div>
        <span class="core-count">{{ tn('alerts.rulesCount', statuses.length) }}</span>
      </div>
      <div v-if="statuses.length" class="alert-list">
        <article
          v-for="(status, index) in statuses"
          :key="`${status.metric}-${status.threshold_pct}-${index}`"
          class="alert-row"
          :class="{ active: status.triggered }"
        >
          <span class="alert-icon"><BellRing :size="16" /></span>
          <div class="alert-main">
            <b>{{ t('alerts.ruleSummary', { metric: metricLabel(status.metric), threshold: status.threshold_pct }) }}</b>
            <small>
              {{ t('alerts.ruleDetail', {
                duration: formatDuration(status.duration_s),
                cooldown: status.cooldown_min,
                last: status.last_fired_ms ? formatTime(status.last_fired_ms) : t('alerts.lastNever'),
              }) }}
            </small>
          </div>
          <div class="alert-live">
            <span>{{ Math.round(status.value_pct) }}%</span>
            <span class="alert-track"><i :style="{ width: `${Math.min(100, status.value_pct)}%` }"></i></span>
          </div>
          <span class="alert-state" :class="status.triggered ? 'is-active' : 'is-ok'">
            {{ status.triggered ? t('alerts.stateActive') : t('alerts.stateIdle') }}
          </span>
          <button class="icon-button alert-remove" type="button" :title="t('alerts.delete')" @click="removeRule(index)">
            <Trash2 :size="15" />
          </button>
        </article>
      </div>
      <div v-else class="detail-empty">{{ t('alerts.empty') }}</div>
    </section>

    <section class="detail-table-card">
      <div class="detail-list-heading">
        <div><h2>{{ t('alerts.newRule') }}</h2><p>{{ t('alerts.newRuleHint') }}</p></div>
      </div>
      <form class="alert-form" @submit.prevent="addRule">
        <label class="alert-field">
          <span>{{ t('alerts.metric') }}</span>
          <select v-model="form.metric" class="select-input">
            <option value="cpu">{{ t('metric.cpu') }}</option>
            <option value="ram">{{ t('metric.ram') }}</option>
          </select>
        </label>
        <label class="alert-field">
          <span>{{ t('alerts.threshold') }}</span>
          <input v-model.number="form.threshold_pct" class="number-input" type="number" min="1" max="100" />
        </label>
        <label class="alert-field">
          <span>{{ t('alerts.duration') }}</span>
          <input v-model.number="form.duration_s" class="number-input" type="number" min="1" max="3600" />
        </label>
        <label class="alert-field">
          <span>{{ t('alerts.cooldown') }}</span>
          <input v-model.number="form.cooldown_min" class="number-input" type="number" min="0" max="1440" />
        </label>
        <button class="primary-button" type="submit" :disabled="adding || !store.config">
          <Plus :size="15" /> {{ adding ? t('alerts.adding') : t('alerts.add') }}
        </button>
      </form>
    </section>

    <section class="detail-table-card">
      <div class="detail-list-heading">
        <div><h2>{{ t('alerts.history') }}</h2><p>{{ t('alerts.historyHint') }}</p></div>
        <span class="core-count">{{ history.length }}</span>
      </div>
      <div v-if="history.length" class="alert-history">
        <div v-for="(event, index) in history" :key="`${event.ts_ms}-${index}`" class="alert-history-row">
          <AlertTriangle :size="14" />
          <span>{{ formatTime(event.ts_ms) }}</span>
          <b>{{ metricLabel(event.metric) }} {{ Math.round(event.value_pct) }}%</b>
          <small>{{ t('alerts.historyThreshold', { value: event.threshold_pct }) }}</small>
        </div>
      </div>
      <div v-else class="detail-empty">{{ t('alerts.historyEmpty') }}</div>
    </section>
  </main>
</template>
