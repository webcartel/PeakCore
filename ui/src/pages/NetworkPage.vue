<script setup lang="ts">
import { computed } from 'vue'
import { ArrowDownToLine, ArrowUpFromLine, Network, Radio } from '@lucide/vue'
import { useMetricsStore } from '../stores/metrics'
import { formatRate, t, tn } from '../i18n'

const metrics = useMetricsStore()
const resources = computed(() => metrics.resources)
const interfaces = computed(() => [...(resources.value?.interfaces ?? [])].sort((a, b) => (b.rx_bps + b.tx_bps) - (a.rx_bps + a.tx_bps)))
</script>

<template>
  <div class="resource-page">
    <header class="resource-page-heading">
      <div>
        <div class="eyebrow">{{ t('eyebrow.monitoring') }}</div>
        <h1>{{ t('network.title') }}</h1>
        <p>{{ t('network.subtitle') }}</p>
      </div>
      <div class="sample-status" :class="{ waiting: !resources?.network_available }"><i></i>{{ !resources ? t('common.waiting') : resources.network_available ? t('common.everySecond') : t('network.unavailable') }}</div>
    </header>

    <section class="resource-kpi-grid network-kpi-grid">
      <article class="resource-kpi-card network-receive-card">
        <div class="resource-kpi-label"><span><ArrowDownToLine :size="16" /></span>{{ t('network.rx') }}</div>
        <strong>{{ resources?.network_available ? formatRate(resources.network_rx_bps) : '—' }}</strong>
        <small>{{ resources?.network_available ? t('network.rxHint') : t('network.na') }}</small>
      </article>
      <article class="resource-kpi-card network-send-card">
        <div class="resource-kpi-label"><span><ArrowUpFromLine :size="16" /></span>{{ t('network.tx') }}</div>
        <strong>{{ resources?.network_available ? formatRate(resources.network_tx_bps) : '—' }}</strong>
        <small>{{ resources?.network_available ? t('network.txHint') : t('network.na') }}</small>
      </article>
    </section>

    <section class="network-interface-card">
      <header class="network-list-heading">
        <div><span class="resource-detail-icon"><Network :size="18" /></span><div><strong>{{ t('network.interfaces') }}</strong><small>{{ resources?.network_available ? tn('network.connected', interfaces.length) : t('network.sourceNA') }}</small></div></div>
        <Radio :size="17" class="network-heading-radio" />
      </header>
      <div v-if="interfaces.length" class="network-interface-list">
        <article v-for="item in interfaces" :key="item.id" class="network-interface-row">
          <span class="network-interface-icon"><Network :size="17" /></span>
          <div class="network-interface-name"><strong>{{ item.name || t('network.adapter') }}</strong><small>{{ t('network.index', { n: item.index }) }}</small></div>
          <div class="network-interface-rate receive"><ArrowDownToLine :size="14" /><span>{{ formatRate(item.rx_bps) }}</span></div>
          <div class="network-interface-rate send"><ArrowUpFromLine :size="14" /><span>{{ formatRate(item.tx_bps) }}</span></div>
        </article>
      </div>
      <div v-else class="network-empty">
        <span><Network :size="22" /></span>
        <strong>{{ !resources ? t('network.emptyWaiting') : resources.network_available ? t('network.emptyNone') : t('network.emptyNA') }}</strong>
        <p>{{ !resources ? t('network.emptyWaitingHint') : resources.network_available ? t('network.emptyNoneHint') : t('network.emptyNAHint') }}</p>
      </div>
    </section>
  </div>
</template>
