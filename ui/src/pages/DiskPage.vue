<script setup lang="ts">
import { computed } from 'vue'
import { Activity, ArrowDownToLine, ArrowUpFromLine, HardDrive } from '@lucide/vue'
import { useMetricsStore } from '../stores/metrics'

const metrics = useMetricsStore()
const disk = computed(() => metrics.resources)

function formatRate(value: number) {
  if (value >= 1024 ** 2) return `${(value / 1024 ** 2).toFixed(1)} МБ/с`
  if (value >= 1024) return `${Math.round(value / 1024)} КБ/с`
  return `${value} Б/с`
}
</script>

<template>
  <div class="resource-page">
    <header class="resource-page-heading">
      <div>
        <div class="eyebrow">PEAKCORE / МОНИТОРИНГ СИСТЕМЫ</div>
        <h1>Диск</h1>
        <p>Текущая активность физических дисков</p>
      </div>
      <div class="sample-status" :class="{ waiting: !disk?.disk_available }"><i></i>{{ !disk ? 'Ожидание данных' : disk.disk_available ? 'Обновление каждую секунду' : 'Счётчики диска недоступны' }}</div>
    </header>

    <section class="resource-kpi-grid">
      <article class="resource-kpi-card">
        <div class="resource-kpi-label"><span><Activity :size="16" /></span>АКТИВНОСТЬ ДИСКА</div>
        <strong>{{ disk?.disk_busy_available ? `${disk.disk_busy_pct.toFixed(1)}%` : '—' }}</strong>
        <div class="resource-gauge"><i :style="{ width: `${Math.min(100, disk?.disk_busy_available ? disk.disk_busy_pct : 0)}%` }"></i></div>
        <small>{{ disk?.disk_busy_available ? 'Время занятости устройства' : 'Счётчик активности недоступен' }}</small>
      </article>
      <article class="resource-kpi-card">
        <div class="resource-kpi-label"><span><ArrowDownToLine :size="16" /></span>ЧТЕНИЕ</div>
        <strong>{{ disk?.disk_available ? formatRate(disk.disk_read_bps) : '—' }}</strong>
        <small>{{ disk?.disk_available ? 'Скорость чтения с дисков' : 'Счётчик чтения недоступен' }}</small>
      </article>
      <article class="resource-kpi-card">
        <div class="resource-kpi-label"><span><ArrowUpFromLine :size="16" /></span>ЗАПИСЬ</div>
        <strong>{{ disk?.disk_available ? formatRate(disk.disk_write_bps) : '—' }}</strong>
        <small>{{ disk?.disk_available ? 'Скорость записи на диски' : 'Счётчик записи недоступен' }}</small>
      </article>
    </section>

    <section class="resource-detail-card">
      <div class="resource-detail-icon"><HardDrive :size="21" /></div>
      <div>
        <strong>Все физические диски</strong>
        <p>{{ disk?.disk_available ? 'Показатели чтения, записи и активности собраны через Windows Performance Counters.' : 'Windows не предоставляет счётчики PhysicalDisk на этом компьютере.' }}</p>
      </div>
      <span v-if="disk?.disk_available" class="resource-live-badge"><i></i>LIVE</span>
    </section>
  </div>
</template>
