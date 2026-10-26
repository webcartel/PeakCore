<script setup lang="ts">
import { useRoute } from 'vue-router'
import { Construction } from '@lucide/vue'

const route = useRoute()
const names: Record<string, string> = {
  processes: 'Процессы', cpu: 'CPU', memory: 'Память', disk: 'Диск',
  network: 'Сеть', alerts: 'Алерты', settings: 'Настройки',
}
</script>

<template>
  <section class="placeholder-page">
    <div class="placeholder-icon"><Construction :size="20" /></div>
    <h1>{{ names[String(route.params.section)] ?? 'PeakCore' }}</h1>
    <p>Этот раздел появится на следующем этапе разработки.</p>
  </section>
</template>
