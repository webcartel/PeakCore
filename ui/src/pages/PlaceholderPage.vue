<script setup lang="ts">
import { Construction } from '@lucide/vue'
import { t } from '../i18n'
</script>

<template>
  <section class="placeholder-page">
    <div class="placeholder-icon"><Construction :size="20" /></div>
    <h1>{{ t('placeholder.title') }}</h1>
    <p>{{ t('placeholder.text') }}</p>
  </section>
</template>
