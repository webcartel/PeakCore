<script setup lang="ts">
import { computed } from 'vue'
import { t } from '../i18n'

const props = defineProps<{ cores: number[] }>()

const tiles = computed(() =>
  props.cores.map((value, index) => ({
    index: index + 1,
    load: Math.min(100, Math.max(0, value)),
  })),
)

function level(load: number) {
  if (load < 60) return 'ok'
  if (load < 85) return 'warn'
  return 'crit'
}
</script>

<template>
  <div class="core-heatmap">
    <div
      v-for="tile in tiles"
      :key="tile.index"
      class="core-tile"
      :class="level(tile.load)"
      :title="t('cpu.coreTitle', { index: tile.index, value: Math.round(tile.load) })"
    >
      <span class="core-index">CORE {{ tile.index }}</span>
      <strong>{{ Math.round(tile.load) }}<small>%</small></strong>
      <i><b :style="{ width: `${tile.load}%` }"></b></i>
    </div>
    <p v-if="!tiles.length" class="core-heatmap-empty">{{ t('cpu.coreEmpty') }}</p>
  </div>
</template>
