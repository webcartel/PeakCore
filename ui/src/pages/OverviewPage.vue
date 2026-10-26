<script setup lang="ts">
import { computed, ref } from 'vue'
import { ArrowUpRight, Cpu, MemoryStick, MoreHorizontal, Radio } from '@lucide/vue'
import { useMetricsStore, type HistoryPoint } from '../stores/metrics'

const metrics = useMetricsStore()
type Period = '1m' | '5m' | '15m' | '1h'
const selectedPeriod = ref<Period>('1m')
const pointLimit = computed(() => ({ '1m': 30, '5m': 150, '15m': 450, '1h': 1800 })[selectedPeriod.value])
const visibleHistory = computed(() => metrics.history.slice(-pointLimit.value))
const timeLabels = computed(() => ({
  '1m': ['−60с', '−45с', '−30с', '−15с', 'Сейчас'],
  '5m': ['−5м', '−3м 45с', '−2м 30с', '−1м 15с', 'Сейчас'],
  '15m': ['−15м', '−11м 15с', '−7м 30с', '−3м 45с', 'Сейчас'],
  '1h': ['−60м', '−45м', '−30м', '−15м', 'Сейчас'],
})[selectedPeriod.value])
const spark = '0,46 20,42 40,45 60,30 80,35 100,24 120,30 140,18 160,24 180,13 200,19 220,10 240,16 260,7 280,12 300,4 320,11 342,3'
const bars = computed(() => metrics.history.length
  ? metrics.history.slice(-24).map((point) => Math.max(8, Math.round(point.cpu * .82)))
  : [38, 54, 43, 68, 48, 76, 60, 35, 57, 83, 51, 67, 41, 70, 49, 91, 58, 44, 72, 53, 80, 47, 63, 39])
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
const cpuLine = computed(() => historyPath('cpu', 800, 190) || 'M0 137 C30 128 38 140 67 127 S102 137 133 112 S169 119 201 101 S238 116 267 95 S304 109 333 83 S372 98 401 77 S435 93 467 69 S506 81 535 61 S573 79 600 54 S640 70 667 45 S706 64 735 35 S771 51 800 28')
const cpuArea = computed(() => cpuLine.value + ' L800 190 L0 190 Z')
const memoryLine = computed(() => historyPath('mem_pct', 800, 190) || 'M0 91 C30 90 38 96 67 89 S102 93 133 83 S169 90 201 78 S238 84 267 73 S304 79 333 71 S372 80 401 65 S435 75 467 62 S506 69 535 58 S573 64 600 55 S640 61 667 49 S706 56 735 43 S771 51 800 38')

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

function historyPath(key: 'cpu' | 'mem_pct', width: number, height: number) {
  const points = visibleHistory.value.slice(-90)
  if (points.length < 2) return ''
  return points.map((point, index) => {
    const x = Math.round(index * width / (points.length - 1))
    const value = Math.max(0, Math.min(100, point[key]))
    const y = Math.round((100 - value) * (height - 8) / 100 + 4)
    return (index === 0 ? 'M' : 'L') + x + ' ' + y
  }).join(' ')
}

function sparkPoints(key: 'cpu' | 'mem_pct') {
  const points = visibleHistory.value.slice(-30)
  if (points.length < 2) return key === 'cpu' ? spark : '0,42 24,40 48,42 72,35 96,37 120,31 144,34 168,27 192,30 216,23 240,26 264,20 288,24 312,16 342,12'
  return points.map((point, index) => {
    const x = Math.round(index * 342 / (points.length - 1))
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
        <svg class="sparkline cpu-spark" viewBox="0 0 342 58" preserveAspectRatio="none"><defs><linearGradient id="cpu-fill" x1="0" x2="0" y1="0" y2="1"><stop offset="0" stop-color="var(--accent)" stop-opacity=".28"/><stop offset="1" stop-color="var(--accent)" stop-opacity="0"/></linearGradient></defs><polygon :points="'0,58 ' + cpuSpark + ' 342,58'" fill="url(#cpu-fill)"/><polyline :points="cpuSpark" fill="none" stroke="var(--accent)" stroke-width="2.5" vector-effect="non-scaling-stroke"/></svg>
      </article>
      <article class="metric-card memory-card">
        <div class="card-top"><span class="metric-label"><span class="metric-icon memory-icon"><MemoryStick :size="15" /></span>ПАМЯТЬ</span><button class="more-button"><MoreHorizontal :size="17" /></button></div>
        <div class="metric-value">{{ memoryPercent }}<span>%</span></div><div class="metric-foot"><strong>{{ memoryUsed }}</strong><span>из {{ memoryTotal }}</span></div>
        <svg class="sparkline memory-spark" viewBox="0 0 342 58" preserveAspectRatio="none"><defs><linearGradient id="mem-fill" x1="0" x2="0" y1="0" y2="1"><stop offset="0" stop-color="var(--secondary)" stop-opacity=".23"/><stop offset="1" stop-color="var(--secondary)" stop-opacity="0"/></linearGradient></defs><polygon :points="'0,58 ' + memorySpark + ' 342,58'" fill="url(#mem-fill)"/><polyline :points="memorySpark" fill="none" stroke="var(--secondary)" stroke-width="2.5" vector-effect="non-scaling-stroke"/></svg>
      </article>
      <article class="metric-card compact-card"><div class="card-top"><span class="metric-label"><span class="metric-icon uptime-icon"><Radio :size="16" /></span>ПРОЦЕССЫ</span><button class="more-button"><MoreHorizontal :size="18" /></button></div><div class="metric-value small-value">{{ metrics.lastUpdated ? metrics.processes.length : '—' }}</div><div class="metric-foot"><span>{{ metrics.lastUpdated ? 'активных процессов' : 'ожидание выборки' }}</span></div><div class="mini-bars"><i v-for="(height, index) in bars" :key="index" :style="{ height: height + '%' }"></i></div></article>
    </div>
    <div class="section-heading"><div><div class="eyebrow">В РЕАЛЬНОМ ВРЕМЕНИ</div><h2>Нагрузка системы</h2></div><span class="live-chip"><i></i> LIVE</span></div>
    <section class="chart-card">
      <div class="chart-legend"><span><i class="legend-cpu"></i>CPU <strong>{{ Math.round(cpu) }}%</strong></span><span><i class="legend-mem"></i>Память <strong>{{ memoryPercent }}%</strong></span></div>
      <div class="chart-area"><div class="chart-y-labels"><span>100%</span><span>75%</span><span>50%</span><span>25%</span><span>0%</span></div><div class="chart-canvas"><div class="chart-grid"><i></i><i></i><i></i><i></i><i></i></div><svg viewBox="0 0 800 190" preserveAspectRatio="none"><defs><linearGradient id="area-cpu" x1="0" x2="0" y1="0" y2="1"><stop offset="0" stop-color="var(--accent)" stop-opacity=".22"/><stop offset="1" stop-color="var(--accent)" stop-opacity="0"/></linearGradient></defs><path :d="cpuArea" fill="url(#area-cpu)"/><path :d="cpuLine" fill="none" stroke="var(--accent)" stroke-width="2.8" vector-effect="non-scaling-stroke"/><path :d="memoryLine" fill="none" stroke="var(--secondary)" stroke-opacity=".9" stroke-width="2.4" vector-effect="non-scaling-stroke"/></svg></div></div>
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
