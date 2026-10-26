<script setup lang="ts">
import { computed, ref } from 'vue'
import { ArrowUpRight, Cpu, MemoryStick, MoreHorizontal, Radio } from '@lucide/vue'
import { useMetricsStore, type HistoryPoint } from '../stores/metrics'

const metrics = useMetricsStore()
type Period = '1m' | '5m' | '15m' | '1h'
const selectedPeriod = ref<Period>('1m')
const periodMs = computed(() => ({ '1m': 60_000, '5m': 5 * 60_000, '15m': 15 * 60_000, '1h': 60 * 60_000 })[selectedPeriod.value])
const visibleHistory = computed(() => {
  const latest = metrics.history.at(-1)?.ts_ms ?? Date.now()
  const cutoff = latest - periodMs.value
  return metrics.history.filter((point) => point.ts_ms >= cutoff && point.ts_ms <= latest)
})
const timeLabels = computed(() => ({
  '1m': ['−60с', '−45с', '−30с', '−15с', 'Сейчас'],
  '5m': ['−5м', '−3м 45с', '−2м 30с', '−1м 15с', 'Сейчас'],
  '15m': ['−15м', '−11м 15с', '−7м 30с', '−3м 45с', 'Сейчас'],
  '1h': ['−60м', '−45м', '−30м', '−15м', 'Сейчас'],
})[selectedPeriod.value])
const bars = computed(() => sampleHistory(24).map((point) => Math.max(8, Math.round(point.cpu * .82))))
const demoProcesses = [
  { name: 'Code.exe', detail: 'Visual Studio Code', cpu: '4.8%', mem: '1.24 ГБ', color: 'purple', width: 72 },
  { name: 'chrome.exe', detail: 'Google Chrome · 8 процессов', cpu: '3.2%', mem: '3.86 ГБ', color: 'blue', width: 49 },
  { name: 'dwm.exe', detail: 'Диспетчер окон рабочего стола', cpu: '1.1%', mem: '186 МБ', color: 'cyan', width: 25 },
]
const cpu = computed(() => metrics.snapshot?.cpu_total ?? 12)
const cpuSpark = computed(() => sparkPoints('cpu'))
const memorySpark = computed(() => sparkPoints('mem_pct'))
const memoryPercent = computed(() => {
  const mem = metrics.snapshot?.mem
  return mem?.total ? Math.round((mem.used / mem.total) * 100) : 68
})
const memoryUsed = computed(() => metrics.snapshot ? formatBytes(metrics.snapshot.mem.used) : '21.8 ГБ')
const memoryTotal = computed(() => metrics.snapshot ? formatBytes(metrics.snapshot.mem.total) : '32 ГБ')
const processRows = computed(() => metrics.processes.length
  ? [...metrics.processes]
      .sort((a, b) => b.cpu - a.cpu)
      .slice(0, 3)
      .map((process, index) => ({
        name: process.name,
        detail: 'PID ' + process.pid + ' · ' + process.threads + ' потоков',
        cpu: process.cpu.toFixed(1) + '%',
        mem: formatBytes(process.private_bytes),
        color: ['purple', 'blue', 'cyan'][index],
        width: Math.min(100, process.cpu * 10),
      }))
  : demoProcesses)
const cpuLine = computed(() => historyPath('cpu', 800, 190))
const cpuArea = computed(() => cpuLine.value + ' L800 190 L0 190 Z')
const memoryLine = computed(() => historyPath('mem_pct', 800, 190))

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

function historyPath(key: 'cpu' | 'mem_pct', width: number, height: number) {
  const points = sampleHistory(300)
  if (points.length < 2) return ''
  const windowStart = (visibleHistory.value.at(-1)?.ts_ms ?? Date.now()) - periodMs.value
  return points.map((point, index) => {
    const x = Math.round((point.ts_ms - windowStart) * width / periodMs.value)
    const value = Math.max(0, Math.min(100, point[key]))
    const y = Math.round((100 - value) * (height - 8) / 100 + 4)
    return (index === 0 ? 'M' : 'L') + x + ' ' + y
  }).join(' ')
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

function formatBytes(value: number) {
  if (value >= 1024 ** 3) return (value / 1024 ** 3).toFixed(1) + ' ГБ'
  if (value >= 1024 ** 2) return Math.round(value / 1024 ** 2) + ' МБ'
  return Math.round(value / 1024) + ' КБ'
}
</script>

<template>
  <div class="overview-page">
    <div class="page-heading">
      <div><div class="eyebrow">PEAKCORE / МОНИТОРИНГ СИСТЕМЫ</div><h1>Обзор системы</h1><p>Ключевые показатели и текущая нагрузка</p></div>
      <div class="period-control"><button :class="{ selected: selectedPeriod === '1m' }" @click="selectPeriod('1m')">1м</button><button :class="{ selected: selectedPeriod === '5m' }" @click="selectPeriod('5m')">5м</button><button :class="{ selected: selectedPeriod === '15m' }" @click="selectPeriod('15m')">15м</button><button :class="{ selected: selectedPeriod === '1h' }" @click="selectPeriod('1h')">1ч</button></div>
    </div>
    <div class="metric-grid">
      <article class="metric-card cpu-card">
        <div class="card-top"><span class="metric-label"><span class="metric-icon cpu-icon"><Cpu :size="15" /></span>ЗАГРУЗКА CPU</span><button class="more-button"><MoreHorizontal :size="17" /></button></div>
        <div class="metric-value">{{ Math.round(cpu) }}<span>%</span></div><div class="metric-foot"><span class="trend positive"><ArrowUpRight :size="13" /> LIVE</span><span>{{ metrics.lastUpdated ? (metrics.daemonConnected ? 'системный snapshot' : 'локальная выборка') : 'ожидание данных' }}</span></div>
        <svg class="sparkline cpu-spark" viewBox="0 0 342 58" preserveAspectRatio="none"><defs><linearGradient id="cpu-fill" x1="0" x2="0" y1="0" y2="1"><stop offset="0" stop-color="var(--accent)" stop-opacity=".28"/><stop offset="1" stop-color="var(--accent)" stop-opacity="0"/></linearGradient></defs><polygon v-if="cpuSpark" :points="'0,58 ' + cpuSpark + ' 342,58'" fill="url(#cpu-fill)"/><polyline v-if="cpuSpark" :points="cpuSpark" fill="none" stroke="var(--accent)" stroke-width="2.5" vector-effect="non-scaling-stroke"/></svg>
      </article>
      <article class="metric-card memory-card">
        <div class="card-top"><span class="metric-label"><span class="metric-icon memory-icon"><MemoryStick :size="15" /></span>ПАМЯТЬ</span><button class="more-button"><MoreHorizontal :size="17" /></button></div>
        <div class="metric-value">{{ memoryPercent }}<span>%</span></div><div class="metric-foot"><strong>{{ memoryUsed }}</strong><span>из {{ memoryTotal }}</span></div>
        <svg class="sparkline memory-spark" viewBox="0 0 342 58" preserveAspectRatio="none"><defs><linearGradient id="mem-fill" x1="0" x2="0" y1="0" y2="1"><stop offset="0" stop-color="var(--secondary)" stop-opacity=".23"/><stop offset="1" stop-color="var(--secondary)" stop-opacity="0"/></linearGradient></defs><polygon v-if="memorySpark" :points="'0,58 ' + memorySpark + ' 342,58'" fill="url(#mem-fill)"/><polyline v-if="memorySpark" :points="memorySpark" fill="none" stroke="var(--secondary)" stroke-width="2.5" vector-effect="non-scaling-stroke"/></svg>
      </article>
      <article class="metric-card compact-card"><div class="card-top"><span class="metric-label"><span class="metric-icon uptime-icon"><Radio :size="16" /></span>ПРОЦЕССЫ</span><button class="more-button"><MoreHorizontal :size="18" /></button></div><div class="metric-value small-value">{{ metrics.lastUpdated ? metrics.processes.length : '—' }}</div><div class="metric-foot"><span>{{ metrics.lastUpdated ? 'активных процессов' : 'ожидание выборки' }}</span></div><div class="mini-bars"><i v-for="(height, index) in bars" :key="index" :style="{ height: height + '%' }"></i></div></article>
    </div>
    <div class="section-heading"><div><div class="eyebrow">В РЕАЛЬНОМ ВРЕМЕНИ</div><h2>Нагрузка системы</h2></div><span class="live-chip"><i></i> LIVE</span></div>
    <section class="chart-card">
      <div class="chart-legend"><span><i class="legend-cpu"></i>CPU <strong>{{ Math.round(cpu) }}%</strong></span><span><i class="legend-mem"></i>Память <strong>{{ memoryPercent }}%</strong></span></div>
      <div class="chart-area"><div class="chart-y-labels"><span>100%</span><span>75%</span><span>50%</span><span>25%</span><span>0%</span></div><div class="chart-canvas"><div class="chart-grid"><i></i><i></i><i></i><i></i><i></i></div><svg viewBox="0 0 800 190" preserveAspectRatio="none"><defs><linearGradient id="area-cpu" x1="0" x2="0" y1="0" y2="1"><stop offset="0" stop-color="var(--accent)" stop-opacity=".22"/><stop offset="1" stop-color="var(--accent)" stop-opacity="0"/></linearGradient></defs><path v-if="cpuLine" :d="cpuArea" fill="url(#area-cpu)"/><path v-if="cpuLine" :d="cpuLine" fill="none" stroke="var(--accent)" stroke-width="2.8" vector-effect="non-scaling-stroke"/><path v-if="memoryLine" :d="memoryLine" fill="none" stroke="var(--secondary)" stroke-opacity=".9" stroke-width="2.4" vector-effect="non-scaling-stroke"/></svg><div v-if="visibleHistory.length < 2" class="chart-placeholder">Собираем данные для графика…</div></div></div>
      <div class="chart-x-labels"><span v-for="label in timeLabels" :key="label">{{ label }}</span></div>
    </section>
    <div class="section-heading process-heading"><div><div class="eyebrow">СЕЙЧАС</div><h2>Топ процессов</h2></div><RouterLink to="/processes" class="view-all">Все процессы <ArrowUpRight :size="14" /></RouterLink></div>
    <section class="process-preview">
      <div class="process-row process-header"><span>ПРОЦЕСС</span><span>CPU</span><span>ПАМЯТЬ</span></div>
      <div v-for="item in processRows" :key="item.name" class="process-row"><div class="process-name"><div class="app-avatar" :class="item.color">{{ item.name.slice(0, 1) }}</div><div><strong>{{ item.name }}</strong><small>{{ item.detail }}</small></div></div><div class="process-stat"><span>{{ item.cpu }}</span><i><b :style="{ width: item.width + '%' }"></b></i></div><div class="memory-stat">{{ item.mem }}</div></div>
    </section>
    <footer class="page-footer"><span>{{ metrics.lastUpdated ? 'Обновлено ' + new Date(metrics.lastUpdated).toLocaleTimeString('ru-RU') : 'Демо-данные · ожидание первого снимка' }}<template v-if="metrics.error"> · {{ metrics.error }}</template></span><span>{{ metrics.processes.length || 3 }} процессов</span></footer>
  </div>
</template>
