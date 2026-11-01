<script setup lang="ts">
import { computed } from 'vue'
import { Activity, ArrowDownToLine, ArrowUpFromLine, HardDrive } from '@lucide/vue'
import { useMetricsStore } from '../stores/metrics'
import { formatRate, t } from '../i18n'

const metrics = useMetricsStore()
const disk = computed(() => metrics.resources)
</script>

<template>
  <div class="resource-page">
    <header class="resource-page-heading">
      <div>
        <div class="eyebrow">{{ t('eyebrow.monitoring') }}</div>
        <h1>{{ t('disk.title') }}</h1>
        <p>{{ t('disk.subtitle') }}</p>
      </div>
      <div class="sample-status" :class="{ waiting: !disk?.disk_available }"><i></i>{{ !disk ? t('common.waiting') : disk.disk_available ? t('common.everySecond') : t('disk.unavailable') }}</div>
    </header>

    <section class="resource-kpi-grid">
      <article class="resource-kpi-card">
        <div class="resource-kpi-label"><span><Activity :size="16" /></span>{{ t('disk.busy') }}</div>
        <strong>{{ disk?.disk_busy_available ? `${disk.disk_busy_pct.toFixed(1)}%` : '—' }}</strong>
        <div class="resource-gauge"><i :style="{ width: `${Math.min(100, disk?.disk_busy_available ? disk.disk_busy_pct : 0)}%` }"></i></div>
        <small>{{ disk?.disk_busy_available ? t('disk.busyHint') : t('disk.busyNA') }}</small>
      </article>
      <article class="resource-kpi-card">
        <div class="resource-kpi-label"><span><ArrowDownToLine :size="16" /></span>{{ t('disk.read') }}</div>
        <strong>{{ disk?.disk_available ? formatRate(disk.disk_read_bps) : '—' }}</strong>
        <small>{{ disk?.disk_available ? t('disk.readHint') : t('disk.readNA') }}</small>
      </article>
      <article class="resource-kpi-card">
        <div class="resource-kpi-label"><span><ArrowUpFromLine :size="16" /></span>{{ t('disk.write') }}</div>
        <strong>{{ disk?.disk_available ? formatRate(disk.disk_write_bps) : '—' }}</strong>
        <small>{{ disk?.disk_available ? t('disk.writeHint') : t('disk.writeNA') }}</small>
      </article>
    </section>

    <section class="resource-detail-card">
      <div class="resource-detail-icon"><HardDrive :size="21" /></div>
      <div>
        <strong>{{ t('disk.all') }}</strong>
        <p>{{ disk?.disk_available ? t('disk.allHint') : t('disk.allNA') }}</p>
      </div>
      <span v-if="disk?.disk_available" class="resource-live-badge"><i></i>{{ t('common.live') }}</span>
    </section>
  </div>
</template>
