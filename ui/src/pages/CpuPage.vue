<script setup lang="ts">
import { computed } from 'vue'
import { Cpu, Gauge, Layers3, Activity } from '@lucide/vue'
import CoreHeatmap from '../components/CoreHeatmap.vue'
import AreaChart from '../components/AreaChart.vue'
import { useMetricsStore } from '../stores/metrics'
import { formatTime, t, tn } from '../i18n'

const metrics = useMetricsStore()
const snapshot = computed(() => metrics.snapshot)
const cpu = computed(() => snapshot.value?.cpu_total ?? 0)
const coreLoads = computed(() => snapshot.value?.cpu_cores ?? [])
const cores = computed(() => coreLoads.value.length || (navigator.hardwareConcurrency || 0))
const sortedProcesses = computed(() => [...metrics.processes].sort((a, b) => b.cpu - a.cpu).slice(0, 6))
const chartPoints = computed(() => metrics.history.slice(-180))
const chartTimestamps = computed(() => chartPoints.value.map((point) => point.ts_ms))
const chartSeries = computed(() => [
  {
    label: 'CPU',
    color: '#ff7841',
    fill: 'rgba(255, 120, 65, .18)',
    values: chartPoints.value.map((point) => point.cpu),
  },
])
const average = computed(() => {
  const points = metrics.history.slice(-60)
  return points.length ? points.reduce((sum, p) => sum + p.cpu, 0) / points.length : cpu.value
})
const peak = computed(() => Math.max(cpu.value, ...metrics.history.slice(-60).map(p => p.cpu)))
const updated = computed(() => metrics.lastUpdated ? formatTime(metrics.lastUpdated) : t('cpu.waitingSnapshot'))
function percent(value: number) { return `${Math.round(value)}%` }
</script>

<template>
  <main class="resource-page detail-page">
    <header class="resource-page-heading"><div><div class="eyebrow">{{ t('eyebrow.processor') }}</div><h1>{{ t('cpu.title') }}</h1><p>{{ t('cpu.subtitle') }}</p></div><span class="resource-live-badge"><i></i>{{ metrics.daemonConnected ? t('common.live') : t('common.snapshot') }}</span></header>
    <section class="resource-kpi-grid detail-kpi-grid">
      <article class="resource-kpi-card cpu-kpi"><div class="resource-kpi-label"><span><Cpu :size="17" /></span>{{ t('cpu.total') }}</div><strong>{{ percent(cpu) }}</strong><small>{{ t('cpu.totalHint') }}</small><div class="resource-gauge"><i :style="{ width: percent(cpu) }"></i></div></article>
      <article class="resource-kpi-card"><div class="resource-kpi-label"><span><Layers3 :size="17" /></span>{{ t('cpu.logical') }}</div><strong>{{ cores || '—' }}</strong><small>{{ t('cpu.logicalHint') }}</small></article>
      <article class="resource-kpi-card"><div class="resource-kpi-label"><span><Gauge :size="17" /></span>{{ t('cpu.average') }}</div><strong>{{ Math.round(average) }}<small class="unit">{{ t('cpu.averageUnit') }}</small></strong><small>{{ t('cpu.peak', { value: percent(peak) }) }}</small></article>
    </section>
    <section class="detail-chart-card">
      <div class="detail-card-heading"><div><span class="detail-icon"><Activity :size="17" /></span><div><h2>{{ t('cpu.history') }}</h2><p>{{ t('cpu.historyHint') }}</p></div></div><b>{{ percent(cpu) }}</b></div>
      <div class="detail-chart-body">
        <AreaChart v-if="chartPoints.length >= 2" :timestamps="chartTimestamps" :series="chartSeries" :height="210" />
        <div v-else class="detail-chart-empty">{{ t('cpu.chartEmpty') }}</div>
      </div>
    </section>
    <section class="detail-table-card">
      <div class="detail-list-heading"><div><h2>{{ t('cpu.heatmap') }}</h2><p>{{ t('cpu.heatmapHint') }}</p></div><span class="core-count">{{ tn('cpu.coresCount', cores) }}</span></div>
      <CoreHeatmap :cores="coreLoads" />
    </section>
    <section class="detail-table-card"><div class="detail-list-heading"><div><h2>{{ t('cpu.topProcesses') }}</h2><p>{{ t('cpu.topHint') }}</p></div><RouterLink to="/processes">{{ t('cpu.allProcesses') }}</RouterLink></div>
      <div v-if="sortedProcesses.length" class="detail-process-list"><div v-for="(proc, index) in sortedProcesses" :key="proc.pid" class="detail-process-row"><span class="detail-rank">{{ String(index + 1).padStart(2, '0') }}</span><div class="detail-process-name"><strong>{{ proc.name }}</strong><small>{{ t('processes.threadsHandles', { threads: proc.threads, handles: proc.handles }) }} · PID {{ proc.pid }}</small></div><div class="detail-process-track"><i :style="{ width: `${Math.min(100, proc.cpu)}%` }"></i></div><b>{{ proc.cpu.toFixed(1) }}%</b></div></div>
      <div v-else class="detail-empty">{{ t('cpu.noProcesses') }}</div>
    </section>
    <footer class="page-footer"><span>{{ t('common.updated', { time: updated }) }}<template v-if="metrics.error"> · {{ metrics.error }}</template></span><span>{{ t('cpu.footerRight') }}</span></footer>
  </main>
</template>
