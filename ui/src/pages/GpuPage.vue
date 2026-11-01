<script setup lang="ts">
import { computed } from 'vue'
import { Activity, CircuitBoard, Gauge, HardDrive } from '@lucide/vue'
import { useMetricsStore } from '../stores/metrics'
import { formatBytes, t } from '../i18n'

const metrics = useMetricsStore()
const gpu = computed(() => metrics.resources)
const engines = computed(() => gpu.value?.gpu_engines ?? [])
const busiest = computed(() => engines.value[0]?.name ?? '—')
const available = computed(() => Boolean(gpu.value?.gpu_available))
</script>

<template>
  <div class="resource-page">
    <header class="resource-page-heading">
      <div>
        <div class="eyebrow">{{ t('eyebrow.gpu') }}</div>
        <h1>{{ t('gpu.title') }}</h1>
        <p>{{ t('gpu.subtitle') }}</p>
      </div>
      <div class="sample-status" :class="{ waiting: !available }">
        <i></i>{{ !gpu ? t('common.waiting') : available ? t('common.everySecond') : t('gpu.unavailable') }}
      </div>
    </header>

    <section class="resource-kpi-grid gpu-kpi-grid">
      <article class="resource-kpi-card">
        <div class="resource-kpi-label"><span><Gauge :size="16" /></span>{{ t('gpu.load') }}</div>
        <strong>{{ available ? `${(gpu?.gpu_utilization_pct ?? 0).toFixed(1)}%` : '—' }}</strong>
        <div class="resource-gauge"><i :style="{ width: `${Math.min(100, gpu?.gpu_utilization_pct ?? 0)}%` }"></i></div>
        <small>{{ available ? t('gpu.loadHint', { engine: busiest }) : t('gpu.unavailable') }}</small>
      </article>
      <article class="resource-kpi-card">
        <div class="resource-kpi-label"><span><HardDrive :size="16" /></span>{{ t('gpu.memory') }}</div>
        <strong>{{ available ? formatBytes(gpu?.gpu_memory_used ?? 0) : '—' }}</strong>
        <small>{{ available ? t('gpu.memoryHint') : t('gpu.unavailable') }}</small>
      </article>
      <article class="resource-kpi-card">
        <div class="resource-kpi-label"><span><CircuitBoard :size="16" /></span>{{ t('gpu.engines') }}</div>
        <strong>{{ available ? engines.length : '—' }}</strong>
        <small>{{ available ? t('gpu.enginesHint') : t('gpu.unavailable') }}</small>
      </article>
    </section>

    <section class="gpu-engine-card">
      <header class="network-list-heading">
        <div>
          <span class="resource-detail-icon"><CircuitBoard :size="18" /></span>
          <div><strong>{{ t('gpu.engineList') }}</strong><small>{{ t('gpu.engineListHint') }}</small></div>
        </div>
        <Activity :size="17" class="network-heading-radio" />
      </header>
      <div v-if="engines.length" class="gpu-engine-list">
        <article v-for="engine in engines" :key="engine.name" class="gpu-engine-row">
          <span class="gpu-engine-name">{{ engine.name }}</span>
          <div class="gpu-engine-track"><i :style="{ width: `${Math.min(100, engine.utilization_pct)}%` }"></i></div>
          <span class="gpu-engine-value">{{ engine.utilization_pct.toFixed(1) }}%</span>
        </article>
      </div>
      <div v-else class="network-empty">
        <span><CircuitBoard :size="22" /></span>
        <strong>{{ gpu ? t('gpu.noEngines') : t('gpu.empty') }}</strong>
        <p>{{ gpu ? t('gpu.unavailable') : t('gpu.engineListHint') }}</p>
      </div>
    </section>

    <section class="resource-detail-card">
      <div class="resource-detail-icon"><Activity :size="21" /></div>
      <div>
        <strong>{{ t('gpu.engineList') }}</strong>
        <p>{{ t('gpu.source') }}</p>
      </div>
      <span v-if="available" class="resource-live-badge"><i></i>{{ t('common.live') }}</span>
    </section>
  </div>
</template>
