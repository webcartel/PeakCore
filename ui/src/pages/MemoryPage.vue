<script setup lang="ts">
import { computed } from 'vue'
import { MemoryStick, Database, Activity } from '@lucide/vue'
import AreaChart from '../components/AreaChart.vue'
import { useMetricsStore } from '../stores/metrics'
import { formatBytes, formatTime, t } from '../i18n'

const metrics = useMetricsStore()
const mem = computed(() => metrics.snapshot?.mem)
const usedPct = computed(() => mem.value?.total ? mem.value.used * 100 / mem.value.total : 0)
const chartPoints = computed(() => metrics.history.slice(-180))
const chartTimestamps = computed(() => chartPoints.value.map((point) => point.ts_ms))
const chartSeries = computed(() => [
  {
    label: t('memory.used'),
    color: '#8ab7ce',
    fill: 'rgba(138, 183, 206, .16)',
    values: chartPoints.value.map((point) => point.mem_pct),
  },
])
const updated = computed(() => metrics.lastUpdated ? formatTime(metrics.lastUpdated) : t('cpu.waitingSnapshot'))
function bytes(value?: number) {
  return value === undefined || !value ? '—' : formatBytes(value)
}
function percent(value?: number, total?: number) { return total ? `${Math.round((value || 0) * 100 / total)}%` : '—' }
</script>

<template>
  <main class="resource-page detail-page">
    <header class="resource-page-heading"><div><div class="eyebrow">{{ t('eyebrow.memory') }}</div><h1>{{ t('memory.title') }}</h1><p>{{ t('memory.subtitle') }}</p></div><span class="resource-live-badge"><i></i>{{ metrics.daemonConnected ? t('common.live') : t('common.snapshot') }}</span></header>
    <section class="resource-kpi-grid detail-kpi-grid memory-kpi-grid">
      <article class="resource-kpi-card memory-primary"><div class="resource-kpi-label"><span><MemoryStick :size="17" /></span>{{ t('memory.used') }}</div><strong>{{ bytes(mem?.used) }}<small class="unit"> / {{ bytes(mem?.total) }}</small></strong><small>{{ t('memory.usedPct', { pct: Math.round(usedPct), free: bytes(mem?.available) }) }}</small><div class="resource-gauge"><i :style="{ width: `${usedPct}%` }"></i></div></article>
      <article class="resource-kpi-card"><div class="resource-kpi-label"><span><Database :size="17" /></span>{{ t('memory.cache') }}</div><strong>{{ bytes(mem?.cached) }}</strong><small>{{ t('memory.cacheHint') }}</small></article>
      <article class="resource-kpi-card"><div class="resource-kpi-label"><span><Activity :size="17" /></span>{{ t('memory.commit') }}</div><strong>{{ bytes(mem?.commit_used) }}<small class="unit"> / {{ bytes(mem?.commit_limit) }}</small></strong><small>{{ t('memory.commitHint', { pct: percent(mem?.commit_used, mem?.commit_limit) }) }}</small><div class="resource-gauge"><i :style="{ width: percent(mem?.commit_used, mem?.commit_limit) }"></i></div></article>
    </section>
    <section class="detail-chart-card">
      <div class="detail-card-heading"><div><span class="detail-icon memory-detail-icon"><Activity :size="17" /></span><div><h2>{{ t('memory.usage') }}</h2><p>{{ t('memory.usageHint') }}</p></div></div><b>{{ Math.round(usedPct) }}%</b></div>
      <div class="detail-chart-body">
        <AreaChart v-if="chartPoints.length >= 2" :timestamps="chartTimestamps" :series="chartSeries" :height="210" />
        <div v-else class="detail-chart-empty">{{ t('cpu.chartEmpty') }}</div>
      </div>
    </section>
    <section class="detail-table-card memory-breakdown"><div class="detail-list-heading"><div><h2>{{ t('memory.breakdown') }}</h2><p>{{ t('memory.cacheHint') }}</p></div></div>
      <div class="memory-breakdown-grid"><article><span class="memory-breakdown-icon"><MemoryStick :size="17" /></span><div><small>{{ t('memory.available') }}</small><strong>{{ bytes(mem?.available) }}</strong></div><b>{{ percent(mem?.available, mem?.total) }}</b></article><article><span class="memory-breakdown-icon"><Database :size="17" /></span><div><small>{{ t('memory.pagefile') }}</small><strong>{{ bytes(mem?.pagefile_used) }} <em>/ {{ bytes(mem?.pagefile_total) }}</em></strong></div><b>{{ percent(mem?.pagefile_used, mem?.pagefile_total) }}</b></article></div>
    </section>
    <footer class="page-footer"><span>{{ t('common.updated', { time: updated }) }}<template v-if="metrics.error"> · {{ metrics.error }}</template></span><span>{{ t('memory.footerRight') }}</span></footer>
  </main>
</template>
