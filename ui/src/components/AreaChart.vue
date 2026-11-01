<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from 'vue'
import uPlot from 'uplot'
import 'uplot/dist/uPlot.min.css'
import { localeCode } from '../i18n'

export interface ChartSeries {
  label: string
  color: string
  fill?: string
  values: number[]
}

const props = withDefaults(
  defineProps<{ timestamps: number[]; series: ChartSeries[]; height?: number }>(),
  { height: 190 },
)

const host = ref<HTMLDivElement | null>(null)
const tip = ref<HTMLDivElement | null>(null)
let chart: uPlot | null = null
let observer: ResizeObserver | null = null
let themeObserver: MutationObserver | null = null

function aligned(): uPlot.AlignedData {
  return [
    props.timestamps.map((ts) => ts / 1000),
    ...props.series.map((series) => series.values),
  ] as uPlot.AlignedData
}

function options(width: number): uPlot.Options {
  const styles = getComputedStyle(document.documentElement)
  const muted = styles.getPropertyValue('--muted').trim() || '#8b93a7'
  const grid = styles.getPropertyValue('--border').trim() || 'rgba(255,255,255,.06)'
  return {
    width,
    height: props.height,
    padding: [10, 10, 0, 0],
    cursor: { y: false, points: { size: 6 }, focus: { prox: 28 } },
    legend: { show: false },
    scales: { x: { time: true }, y: { range: [0, 100] } },
    axes: [
      {
        stroke: muted,
        grid: { stroke: grid },
        ticks: { stroke: grid },
        font: '10px Consolas, monospace',
        size: 34,
      },
      {
        stroke: muted,
        grid: { stroke: grid },
        ticks: { stroke: grid },
        font: '10px Consolas, monospace',
        size: 42,
        values: (_u: uPlot, splits: number[]) => splits.map((value) => `${value}%`),
      },
    ],
    series: [
      { label: 'time' },
      ...props.series.map((series) => ({
        label: series.label,
        stroke: series.color,
        fill: series.fill ?? 'transparent',
        width: 2.4,
        points: { show: false },
      })),
    ],
    hooks: {
      setCursor: [(self: uPlot) => showTip(self)],
    },
  }
}

function showTip(self: uPlot) {
  const element = tip.value
  if (!element) return
  const index = self.cursor.idx ?? -1
  const timestamp = props.timestamps[index]
  if (index < 0 || timestamp === undefined) {
    element.style.opacity = '0'
    return
  }
  const rows = props.series
    .map(
      (series) =>
        `<span><i style="background:${series.color}"></i>${series.label} <b>${Math.round(series.values[index] ?? 0)}%</b></span>`,
    )
    .join('')
  element.innerHTML = `<b>${new Date(timestamp).toLocaleTimeString(localeCode())}</b>${rows}`
  element.style.opacity = '1'
  element.style.transform = `translate(${Math.max(0, (self.cursor.left ?? 0) + 14)}px, ${Math.max(0, (self.cursor.top ?? 0) - 12)}px)`
}

function build() {
  const element = host.value
  if (!element) return
  chart?.destroy()
  chart = new uPlot(options(element.clientWidth || 640), aligned(), element)
}

watch(
  () => props.timestamps,
  () => chart?.setData(aligned()),
)
watch(
  () => props.series.map((series) => series.values),
  () => chart?.setData(aligned()),
)
watch(
  () => props.series.map((series) => `${series.label}:${series.color}`).join('|'),
  build,
)

onMounted(() => {
  build()
  observer = new ResizeObserver((entries) => {
    const width = entries[0]?.contentRect.width
    if (chart && width) chart.setSize({ width, height: props.height })
  })
  if (host.value) observer.observe(host.value)
  themeObserver = new MutationObserver(build)
  themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] })
})

onUnmounted(() => {
  observer?.disconnect()
  themeObserver?.disconnect()
  chart?.destroy()
  chart = null
})
</script>

<template>
  <div class="chart-host">
    <div ref="host" class="chart-canvas-host"></div>
    <div ref="tip" class="chart-tip"></div>
  </div>
</template>
