<script setup lang="ts">
import { computed } from 'vue'
import { ArrowDownToLine, ArrowUpFromLine, Network, Radio } from '@lucide/vue'
import { useMetricsStore } from '../stores/metrics'

const metrics = useMetricsStore()
const resources = computed(() => metrics.resources)
const interfaces = computed(() => [...(resources.value?.interfaces ?? [])].sort((a, b) => (b.rx_bps + b.tx_bps) - (a.rx_bps + a.tx_bps)))

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
        <h1>Сеть</h1>
        <p>Скорость передачи данных по сетевым интерфейсам</p>
      </div>
      <div class="sample-status" :class="{ waiting: !resources?.network_available }"><i></i>{{ !resources ? 'Ожидание данных' : resources.network_available ? 'Обновление каждую секунду' : 'Сетевые счётчики недоступны' }}</div>
    </header>

    <section class="resource-kpi-grid network-kpi-grid">
      <article class="resource-kpi-card network-receive-card">
        <div class="resource-kpi-label"><span><ArrowDownToLine :size="16" /></span>ВХОДЯЩИЙ ТРАФИК</div>
        <strong>{{ resources?.network_available ? formatRate(resources.network_rx_bps) : '—' }}</strong>
        <small>{{ resources?.network_available ? 'Суммарная скорость получения' : 'Счётчик сети недоступен' }}</small>
      </article>
      <article class="resource-kpi-card network-send-card">
        <div class="resource-kpi-label"><span><ArrowUpFromLine :size="16" /></span>ИСХОДЯЩИЙ ТРАФИК</div>
        <strong>{{ resources?.network_available ? formatRate(resources.network_tx_bps) : '—' }}</strong>
        <small>{{ resources?.network_available ? 'Суммарная скорость отправки' : 'Счётчик сети недоступен' }}</small>
      </article>
    </section>

    <section class="network-interface-card">
      <header class="network-list-heading">
        <div><span class="resource-detail-icon"><Network :size="18" /></span><div><strong>Сетевые интерфейсы</strong><small>{{ resources?.network_available ? `${interfaces.length} подключённых интерфейсов` : 'Источник данных недоступен' }}</small></div></div>
        <Radio :size="17" class="network-heading-radio" />
      </header>
      <div v-if="interfaces.length" class="network-interface-list">
        <article v-for="item in interfaces" :key="item.id" class="network-interface-row">
          <span class="network-interface-icon"><Network :size="17" /></span>
          <div class="network-interface-name"><strong>{{ item.name || 'Сетевой адаптер' }}</strong><small>Индекс {{ item.index }}</small></div>
          <div class="network-interface-rate receive"><ArrowDownToLine :size="14" /><span>{{ formatRate(item.rx_bps) }}</span></div>
          <div class="network-interface-rate send"><ArrowUpFromLine :size="14" /><span>{{ formatRate(item.tx_bps) }}</span></div>
        </article>
      </div>
      <div v-else class="network-empty">
        <span><Network :size="22" /></span>
        <strong>{{ !resources ? 'Ожидание сетевых данных' : resources.network_available ? 'Активные интерфейсы не найдены' : 'Сетевые счётчики недоступны' }}</strong>
        <p>{{ !resources ? 'Показатели появятся после первого опроса системы.' : resources.network_available ? 'Подключите сетевой адаптер, чтобы увидеть его трафик.' : 'Windows не вернула таблицу сетевых интерфейсов.' }}</p>
      </div>
    </section>
  </div>
</template>
