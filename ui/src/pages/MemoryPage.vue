<script setup lang="ts">
import { computed } from 'vue'
import { MemoryStick, Database, Activity } from '@lucide/vue'
import { useMetricsStore } from '../stores/metrics'

const metrics = useMetricsStore()
const mem = computed(() => metrics.snapshot?.mem)
const usedPct = computed(() => mem.value?.total ? mem.value.used * 100 / mem.value.total : 0)
const chart = computed(() => {
  const points = metrics.history.slice(-90)
  if (points.length < 2) return ''
  return points.map((p, i) => `${i ? 'L' : 'M'}${Math.round(i * 800 / (points.length - 1))} ${Math.round((100 - Math.max(0, Math.min(100, p.mem_pct))) * 1.82 + 4)}`).join(' ')
})
const updated = computed(() => metrics.lastUpdated ? new Date(metrics.lastUpdated).toLocaleTimeString('ru-RU') : 'Ожидание снимка')
function formatBytes(value?: number) {
  if (value === undefined || !value) return '—'
  if (value >= 1024 ** 3) return `${(value / 1024 ** 3).toFixed(1)} ГБ`
  if (value >= 1024 ** 2) return `${Math.round(value / 1024 ** 2)} МБ`
  return `${Math.round(value / 1024)} КБ`
}
function percent(value?: number, total?: number) { return total ? `${Math.round((value || 0) * 100 / total)}%` : '—' }
</script>

<template>
  <main class="resource-page detail-page">
    <header class="resource-page-heading"><div><div class="eyebrow">PEAKCORE / ОПЕРАТИВНАЯ ПАМЯТЬ</div><h1>Память</h1><p>Физическая память, системный commit и файл подкачки</p></div><span class="resource-live-badge"><i></i>{{ metrics.daemonConnected ? 'LIVE' : 'СНИМОК' }}</span></header>
    <section class="resource-kpi-grid detail-kpi-grid memory-kpi-grid">
      <article class="resource-kpi-card memory-primary"><div class="resource-kpi-label"><span><MemoryStick :size="17" /></span>ФИЗИЧЕСКАЯ ПАМЯТЬ</div><strong>{{ formatBytes(mem?.used) }}<small class="unit"> / {{ formatBytes(mem?.total) }}</small></strong><small>Занято {{ Math.round(usedPct) }}% · доступно {{ formatBytes(mem?.available) }}</small><div class="resource-gauge"><i :style="{ width: `${usedPct}%` }"></i></div></article>
      <article class="resource-kpi-card"><div class="resource-kpi-label"><span><Database :size="17" /></span>КЭШ СИСТЕМЫ</div><strong>{{ formatBytes(mem?.cached) }}</strong><small>Системный файловый кэш и standby</small></article>
      <article class="resource-kpi-card"><div class="resource-kpi-label"><span><Activity :size="17" /></span>СИСТЕМНЫЙ COMMIT</div><strong>{{ formatBytes(mem?.commit_used) }}<small class="unit"> / {{ formatBytes(mem?.commit_limit) }}</small></strong><small>Использовано {{ percent(mem?.commit_used, mem?.commit_limit) }} от лимита</small><div class="resource-gauge"><i :style="{ width: percent(mem?.commit_used, mem?.commit_limit) }"></i></div></article>
    </section>
    <section class="detail-chart-card">
      <div class="detail-card-heading"><div><span class="detail-icon memory-detail-icon"><Activity :size="17" /></span><div><h2>Использование физической памяти</h2><p>Доля занятой памяти за историю измерений</p></div></div><b>{{ Math.round(usedPct) }}%</b></div>
      <div class="detail-chart memory-detail-chart"><div class="detail-chart-grid"><i v-for="n in 5" :key="n"></i></div><div class="detail-chart-y"><span>100%</span><span>75%</span><span>50%</span><span>25%</span><span>0%</span></div><svg viewBox="0 0 800 190" preserveAspectRatio="none"><defs><linearGradient id="memAreaFill" x1="0" x2="0" y1="0" y2="1"><stop offset="0" stop-color="var(--secondary)" stop-opacity=".25"/><stop offset="1" stop-color="var(--secondary)" stop-opacity="0"/></linearGradient></defs><path v-if="chart" :d="chart + ' L800 186 L0 186 Z'" fill="url(#memAreaFill)"/><path v-if="chart" :d="chart" fill="none" stroke="var(--secondary)" stroke-width="3" vector-effect="non-scaling-stroke"/></svg><div v-if="!chart" class="detail-chart-empty">График появится после накопления измерений</div></div>
      <div class="detail-chart-labels"><span>Старые данные</span><span>Сейчас</span></div>
    </section>
    <section class="detail-table-card memory-breakdown"><div class="detail-list-heading"><div><h2>Распределение памяти</h2><p>Дополнительные системные счётчики</p></div></div>
      <div class="memory-breakdown-grid"><article><span class="memory-breakdown-icon"><MemoryStick :size="17" /></span><div><small>ДОСТУПНО</small><strong>{{ formatBytes(mem?.available) }}</strong></div><b>{{ percent(mem?.available, mem?.total) }}</b></article><article><span class="memory-breakdown-icon"><Database :size="17" /></span><div><small>ФАЙЛ ПОДКАЧКИ</small><strong>{{ formatBytes(mem?.pagefile_used) }} <em>/ {{ formatBytes(mem?.pagefile_total) }}</em></strong></div><b>{{ percent(mem?.pagefile_used, mem?.pagefile_total) }}</b></article></div>
    </section>
    <footer class="page-footer"><span>Обновлено {{ updated }}<template v-if="metrics.error"> · {{ metrics.error }}</template></span><span>Полный набор счётчиков обновляется раз в 30 секунд</span></footer>
  </main>
</template>
