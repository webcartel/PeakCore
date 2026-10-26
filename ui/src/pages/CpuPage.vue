<script setup lang="ts">
import { computed } from 'vue'
import { Cpu, Gauge, Layers3, Activity } from '@lucide/vue'
import { useMetricsStore } from '../stores/metrics'

const metrics = useMetricsStore()
const snapshot = computed(() => metrics.snapshot)
const cpu = computed(() => snapshot.value?.cpu_total ?? 0)
const cores = navigator.hardwareConcurrency || 0
const sortedProcesses = computed(() => [...metrics.processes].sort((a, b) => b.cpu - a.cpu).slice(0, 6))
const chart = computed(() => {
  const points = metrics.history.slice(-90)
  if (points.length < 2) return ''
  return points.map((p, i) => `${i ? 'L' : 'M'}${Math.round(i * 800 / (points.length - 1))} ${Math.round((100 - Math.max(0, Math.min(100, p.cpu))) * 1.82 + 4)}`).join(' ')
})
const average = computed(() => {
  const points = metrics.history.slice(-60)
  return points.length ? points.reduce((sum, p) => sum + p.cpu, 0) / points.length : cpu.value
})
const peak = computed(() => Math.max(cpu.value, ...metrics.history.slice(-60).map(p => p.cpu)))
const updated = computed(() => metrics.lastUpdated ? new Date(metrics.lastUpdated).toLocaleTimeString('ru-RU') : 'Ожидание снимка')
function percent(value: number) { return `${Math.round(value)}%` }
</script>

<template>
  <main class="resource-page detail-page">
    <header class="resource-page-heading"><div><div class="eyebrow">PEAKCORE / ПРОЦЕССОР</div><h1>Процессор</h1><p>Загрузка системы и вклад активных процессов</p></div><span class="resource-live-badge"><i></i>{{ metrics.daemonConnected ? 'LIVE' : 'СНИМОК' }}</span></header>
    <section class="resource-kpi-grid detail-kpi-grid">
      <article class="resource-kpi-card cpu-kpi"><div class="resource-kpi-label"><span><Cpu :size="17" /></span>ОБЩАЯ ЗАГРУЗКА</div><strong>{{ percent(cpu) }}</strong><small>Текущая загрузка процессора</small><div class="resource-gauge"><i :style="{ width: percent(cpu) }"></i></div></article>
      <article class="resource-kpi-card"><div class="resource-kpi-label"><span><Layers3 :size="17" /></span>ЛОГИЧЕСКИЕ ПРОЦЕССОРЫ</div><strong>{{ cores || '—' }}</strong><small>Логические процессоры, доступные приложению</small></article>
      <article class="resource-kpi-card"><div class="resource-kpi-label"><span><Gauge :size="17" /></span>СРЕДНЯЯ / ПИК</div><strong>{{ Math.round(average) }}<small class="unit">% ср.</small></strong><small>Пик за видимую историю: {{ percent(peak) }}</small></article>
    </section>
    <section class="detail-chart-card">
      <div class="detail-card-heading"><div><span class="detail-icon"><Activity :size="17" /></span><div><h2>История загрузки CPU</h2><p>Снимки за выбранный период</p></div></div><b>{{ percent(cpu) }}</b></div>
      <div class="detail-chart"><div class="detail-chart-grid"><i v-for="n in 5" :key="n"></i></div><div class="detail-chart-y"><span>100%</span><span>75%</span><span>50%</span><span>25%</span><span>0%</span></div><svg viewBox="0 0 800 190" preserveAspectRatio="none"><defs><linearGradient id="cpuAreaFill" x1="0" x2="0" y1="0" y2="1"><stop offset="0" stop-color="var(--accent)" stop-opacity=".28"/><stop offset="1" stop-color="var(--accent)" stop-opacity="0"/></linearGradient></defs><path v-if="chart" :d="chart + ' L800 186 L0 186 Z'" fill="url(#cpuAreaFill)"/><path v-if="chart" :d="chart" fill="none" stroke="var(--accent)" stroke-width="3" vector-effect="non-scaling-stroke"/></svg><div v-if="!chart" class="detail-chart-empty">График появится после накопления измерений</div></div>
      <div class="detail-chart-labels"><span>Старые данные</span><span>Сейчас</span></div>
    </section>
    <section class="detail-table-card"><div class="detail-list-heading"><div><h2>Процессы с наибольшей нагрузкой</h2><p>Текущая загрузка отдельных процессов</p></div><RouterLink to="/processes">Все процессы ↗</RouterLink></div>
      <div v-if="sortedProcesses.length" class="detail-process-list"><div v-for="(proc, index) in sortedProcesses" :key="proc.pid" class="detail-process-row"><span class="detail-rank">{{ String(index + 1).padStart(2, '0') }}</span><div class="detail-process-name"><strong>{{ proc.name }}</strong><small>PID {{ proc.pid }} · {{ proc.threads }} потоков</small></div><div class="detail-process-track"><i :style="{ width: `${Math.min(100, proc.cpu)}%` }"></i></div><b>{{ proc.cpu.toFixed(1) }}%</b></div></div>
      <div v-else class="detail-empty">Данные о процессах появятся после подключения к службе</div>
    </section>
    <footer class="page-footer"><span>Обновлено {{ updated }}<template v-if="metrics.error"> · {{ metrics.error }}</template></span><span>Детализация по каждому ядру пока недоступна</span></footer>
  </main>
</template>
