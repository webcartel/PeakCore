<script setup lang="ts">
import { computed, ref } from 'vue'
import { ArrowUpRight, Cpu, MemoryStick, MoreHorizontal, Radio } from '@lucide/vue'
import AreaChart from '../components/AreaChart.vue'
import Skeleton from '../components/Skeleton.vue'
import { useMetricsStore, type HistoryPoint } from '../stores/metrics'
import { useConfigStore } from '../stores/config'
import { formatBytes, formatTime, t, tn, type MessageKey } from '../i18n'

const metrics = useMetricsStore()
const config = useConfigStore()
type Period = '1m' | '5m' | '15m' | '1h' | '24h'
const periods: Period[] = ['1m', '5m', '15m', '1h', '24h']
const periodKeys: Record<Period, MessageKey> = {
  '1m': 'period.1m',
  '5m': 'period.5m',
  '15m': 'period.15m',
  '1h': 'period.1h',
  '24h': 'period.24h',
}
const selectedPeriod = ref<Period>('1m')
const periodMs = computed(() => ({
  '1m': 60_000,
  '5m': 5 * 60_000,
  '15m': 15 * 60_000,
  '1h': 60 * 60_000,
  '24h': 24 * 60 * 60_000,
})[selectedPeriod.value])
const visibleHistory = computed(() => {
  const latest = metrics.history.at(-1)?.ts_ms ?? Date.now()
  const cutoff = latest - periodMs.value
  return metrics.history.filter((point) => point.ts_ms >= cutoff && point.ts_ms <= latest)
})
const bars = computed(() => sampleHistory(24).map((point) => Math.max(8, Math.round(point.cpu * .82))))
const cpu = computed(() => metrics.snapshot?.cpu_total ?? 12)
const cpuSpark = computed(() => sparkPoints('cpu'))
const memorySpark = computed(() => sparkPoints('mem_pct'))
const memoryPercent = computed(() => {
  const mem = metrics.snapshot?.mem
  return mem?.total ? Math.round((mem.used / mem.total) * 100) : 68
})
const memoryUsed = computed(() => metrics.snapshot ? formatBytes(metrics.snapshot.mem.used) : `21.8 ${t('unit.gb')}`)
const memoryTotal = computed(() => metrics.snapshot ? formatBytes(metrics.snapshot.mem.total) : `32 ${t('unit.gb')}`)
const demoProcesses = computed(() => [
  { name: 'Code.exe', detail: 'Visual Studio Code', cpu: '4.8%', mem: `1.24 ${t('unit.gb')}`, color: 'purple', width: 72 },
  { name: 'chrome.exe', detail: t('overview.demoChrome', { n: 8 }), cpu: '3.2%', mem: `3.86 ${t('unit.gb')}`, color: 'blue', width: 49 },
  { name: 'dwm.exe', detail: t('overview.demoDwm'), cpu: '1.1%', mem: `186 ${t('unit.mb')}`, color: 'cyan', width: 25 },
])
const processRows = computed(() => metrics.processes.length
  ? [...metrics.processes]
      .sort((a, b) => b.cpu - a.cpu)
      .slice(0, 3)
      .map((process, index) => ({
        name: process.name,
        detail: t('overview.pidThreads', { pid: process.pid, threads: process.threads }),
        cpu: process.cpu.toFixed(1) + '%',
        mem: formatBytes(process.private_bytes),
        color: ['purple', 'blue', 'cyan'][index],
        width: Math.min(100, process.cpu * 10),
      }))
  : demoProcesses.value)
const chartPoints = computed(() => isDaily.value ? [] : sampleHistory(360))
const isDaily = computed(() => selectedPeriod.value === '24h')
const dailyMode = ref<'avg' | 'max'>('avg')
const dailyAggregates = computed(() => isDaily.value ? metrics.aggregates : [])
const coverage = computed(() => {
  const interval = config.config?.sampling.tray_interval_ms ?? 2_000
  const expected = Math.max(1, Math.round(60_000 / interval))
  const partial = dailyAggregates.value.filter((item) => item.samples < expected * 0.9).length
  return { expected, partial }
})
const chartTimestamps = computed(() => isDaily.value
  ? dailyAggregates.value.map((item) => item.minute_ts_ms)
  : chartPoints.value.map((point) => point.ts_ms))
const chartSeries = computed(() => [
  {
    label: 'CPU',
    color: '#ff7841',
    fill: 'rgba(255, 120, 65, .16)',
    values: isDaily.value
      ? dailyAggregates.value.map((item) => dailyMode.value === 'max' ? item.cpu_max : item.cpu_avg)
      : chartPoints.value.map((point) => point.cpu),
  },
  {
    label: t('overview.memory'),
    color: '#8ab7ce',
    fill: 'rgba(138, 183, 206, .10)',
    values: isDaily.value
      ? dailyAggregates.value.map((item) => dailyMode.value === 'max' ? item.mem_max : item.mem_avg)
      : chartPoints.value.map((point) => point.mem_pct),
  },
])

async function selectPeriod(period: Period) {
  selectedPeriod.value = period
  if (!('__TAURI_INTERNALS__' in window)) return
  const range = period === '1m' ? '5m' : period
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const points = await invoke<HistoryPoint[]>('set_history_range', { range })
    metrics.replaceHistory(points)
  } catch (error) {
    metrics.error = String(error)
  }
}

function sampleHistory(limit: number) {
  const points = visibleHistory.value
  if (points.length <= limit) return points
  return Array.from({ length: limit }, (_, index) => points[Math.round(index * (points.length - 1) / (limit - 1))])
}

function sparkPoints(key: 'cpu' | 'mem_pct') {
  const points = sampleHistory(48)
  if (points.length < 2) return ''
  const windowStart = (visibleHistory.value.at(-1)?.ts_ms ?? Date.now()) - periodMs.value
  return points.map((point) => {
    const x = Math.round((point.ts_ms - windowStart) * 342 / periodMs.value)
    const value = Math.max(0, Math.min(100, point[key]))
    const y = Math.round((100 - value) * 0.5 + 4)
    return x + ',' + y
  }).join(' ')
}
</script>

<template>
  <div class="overview-page">
    <div class="page-heading">
      <div><div class="eyebrow">{{ t('eyebrow.monitoring') }}</div><h1>{{ t('overview.title') }}</h1><p>{{ t('overview.subtitle') }}</p></div>
      <div class="period-control"><button v-for="period in periods" :key="period" :class="{ selected: selectedPeriod === period }" @click="selectPeriod(period)">{{ t(periodKeys[period]) }}</button></div>
    </div>
    <div class="metric-grid">
      <article class="metric-card cpu-card">
        <div class="card-top"><span class="metric-label"><span class="metric-icon cpu-icon"><Cpu :size="15" /></span>{{ t('overview.cpu') }}</span><button class="more-button"><MoreHorizontal :size="17" /></button></div>
        <div class="metric-value"><template v-if="metrics.lastUpdated">{{ Math.round(cpu) }}<span>%</span></template><Skeleton v-else width="110px" height=".8em" /></div><div class="metric-foot"><span class="trend positive"><ArrowUpRight :size="13" /> {{ t('common.live') }}</span><span>{{ metrics.lastUpdated ? (metrics.daemonConnected ? t('overview.systemSnapshot') : t('overview.localSample')) : t('overview.waitingData') }}</span></div>
        <svg class="sparkline cpu-spark" viewBox="0 0 342 58" preserveAspectRatio="none"><defs><linearGradient id="cpu-fill" x1="0" x2="0" y1="0" y2="1"><stop offset="0" stop-color="var(--accent)" stop-opacity=".28"/><stop offset="1" stop-color="var(--accent)" stop-opacity="0"/></linearGradient></defs><polygon v-if="cpuSpark" :points="'0,58 ' + cpuSpark + ' 342,58'" fill="url(#cpu-fill)"/><polyline v-if="cpuSpark" :points="cpuSpark" fill="none" stroke="var(--accent)" stroke-width="2.5" vector-effect="non-scaling-stroke"/></svg>
      </article>
      <article class="metric-card memory-card">
        <div class="card-top"><span class="metric-label"><span class="metric-icon memory-icon"><MemoryStick :size="15" /></span>{{ t('overview.memory') }}</span><button class="more-button"><MoreHorizontal :size="17" /></button></div>
        <div class="metric-value"><template v-if="metrics.lastUpdated">{{ memoryPercent }}<span>%</span></template><Skeleton v-else width="110px" height=".8em" /></div><div class="metric-foot"><strong>{{ memoryUsed }}</strong><span>{{ t('overview.of') }} {{ memoryTotal }}</span></div>
        <svg class="sparkline memory-spark" viewBox="0 0 342 58" preserveAspectRatio="none"><defs><linearGradient id="mem-fill" x1="0" x2="0" y1="0" y2="1"><stop offset="0" stop-color="var(--secondary)" stop-opacity=".23"/><stop offset="1" stop-color="var(--secondary)" stop-opacity="0"/></linearGradient></defs><polygon v-if="memorySpark" :points="'0,58 ' + memorySpark + ' 342,58'" fill="url(#mem-fill)"/><polyline v-if="memorySpark" :points="memorySpark" fill="none" stroke="var(--secondary)" stroke-width="2.5" vector-effect="non-scaling-stroke"/></svg>
      </article>
      <article class="metric-card compact-card"><div class="card-top"><span class="metric-label"><span class="metric-icon uptime-icon"><Radio :size="16" /></span>{{ t('overview.processes') }}</span><button class="more-button"><MoreHorizontal :size="18" /></button></div><div class="metric-value small-value"><template v-if="metrics.lastUpdated">{{ metrics.processes.length }}</template><Skeleton v-else width="64px" height=".8em" /></div><div class="metric-foot"><span>{{ metrics.lastUpdated ? t('overview.activeProcesses') : t('overview.waitingSample') }}</span></div><div class="mini-bars"><i v-for="(height, index) in bars" :key="index" :style="{ height: height + '%' }"></i></div></article>
    </div>
    <div class="section-heading"><div><div class="eyebrow">{{ t('overview.realtime') }}</div><h2>{{ t('overview.load') }}</h2></div><span class="live-chip"><i></i> {{ t('common.live') }}</span></div>
    <section class="chart-card">
      <div class="chart-card-head">
        <div class="chart-legend"><span><i class="legend-cpu"></i>CPU <strong>{{ Math.round(cpu) }}%</strong></span><span><i class="legend-mem"></i>{{ t('overview.memory') }} <strong>{{ memoryPercent }}%</strong></span></div>
        <div v-if="isDaily" class="segmented chart-mode">
          <button type="button" :class="{ active: dailyMode === 'avg' }" @click="dailyMode = 'avg'">{{ t('overview.average') }}</button>
          <button type="button" :class="{ active: dailyMode === 'max' }" @click="dailyMode = 'max'">{{ t('overview.maximum') }}</button>
        </div>
      </div>
      <div class="chart-body">
        <AreaChart v-if="chartTimestamps.length >= 2" :timestamps="chartTimestamps" :series="chartSeries" :height="200" />
        <div v-else class="chart-placeholder chart-placeholder-static">{{ t('overview.collecting') }}</div>
      </div>
      <p v-if="isDaily && coverage.partial" class="chart-note">{{ t('overview.partialMinutes', { n: coverage.partial }) }}</p>
    </section>
    <div class="section-heading process-heading"><div><div class="eyebrow">{{ t('overview.now') }}</div><h2>{{ t('overview.topProcesses') }}</h2></div><RouterLink to="/processes" class="view-all">{{ t('overview.allProcesses') }} <ArrowUpRight :size="14" /></RouterLink></div>
    <section class="process-preview">
      <div class="process-row process-header"><span>{{ t('overview.col.process') }}</span><span>{{ t('overview.col.cpu') }}</span><span>{{ t('overview.col.memory') }}</span></div>
      <div v-for="item in processRows" :key="item.name" class="process-row"><div class="process-name"><div class="app-avatar" :class="item.color">{{ item.name.slice(0, 1) }}</div><div><strong>{{ item.name }}</strong><small>{{ item.detail }}</small></div></div><div class="process-stat"><span>{{ item.cpu }}</span><i><b :style="{ width: item.width + '%' }"></b></i></div><div class="memory-stat">{{ item.mem }}</div></div>
    </section>
    <footer class="page-footer"><span>{{ metrics.lastUpdated ? `${t('common.updated', { time: formatTime(metrics.lastUpdated) })}` : t('overview.demoData') }}<template v-if="metrics.error"> · {{ metrics.error }}</template></span><span>{{ tn('overview.processCount', metrics.processes.length || 3) }}</span></footer>
  </div>
</template>
