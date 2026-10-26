<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { Activity, ArrowDown, ArrowUp, ChevronDown, Cpu, LayoutDashboard, Search, X } from '@lucide/vue'
import { invoke } from '@tauri-apps/api/core'
import { useMetricsStore, type ProcSample } from '../stores/metrics'

type Mode = 'apps' | 'processes'
type SortKey = 'cpu' | 'memory' | 'name' | 'read' | 'write'
type AppGroup = {
  key: string
  name: string
  members: ProcSample[]
  cpu: number
  privateBytes: number
  workingSet: number
  readBps: number
  writeBps: number
}
type DisplayRow =
  | { type: 'app'; key: string; group: AppGroup }
  | { type: 'child'; key: string; groupKey: string; process: ProcSample }
  | { type: 'process'; key: string; process: ProcSample }

const metrics = useMetricsStore()
const mode = ref<Mode>('apps')
const sortBy = ref<SortKey>('cpu')
const descending = ref(true)
const search = ref('')
const expanded = ref<string[]>([])
const scrollElement = ref<HTMLElement | null>(null)
const scrollTop = ref(0)
const viewportHeight = ref(520)
const rowHeight = 61
const overscan = 8

const virtualStart = computed(() => Math.max(0, Math.floor(scrollTop.value / rowHeight) - overscan))
const virtualEnd = computed(() => Math.min(rows.value.length, Math.ceil((scrollTop.value + viewportHeight.value) / rowHeight) + overscan))
const visibleRows = computed(() => rows.value.slice(virtualStart.value, virtualEnd.value))
const topSpacer = computed(() => virtualStart.value * rowHeight)
const bottomSpacer = computed(() => Math.max(0, (rows.value.length - virtualEnd.value) * rowHeight))
const contextMenu = ref<{ x: number; y: number; process: ProcSample; path?: string; pathLoading: boolean } | null>(null)
const copyStatus = ref('')

function updateViewport() {
  if (!scrollElement.value) return
  scrollTop.value = scrollElement.value.scrollTop
  viewportHeight.value = scrollElement.value.clientHeight
}

function onOutsidePointer(event: PointerEvent) {
  if (!(event.target instanceof Element) || !event.target.closest('.process-context-menu')) contextMenu.value = null
}

function onContextMenu(event: MouseEvent, process: ProcSample) {
  event.preventDefault()
  contextMenu.value = { x: Math.min(event.clientX, window.innerWidth - 220), y: Math.min(event.clientY, window.innerHeight - 110), process, path: process.exe_path ?? undefined, pathLoading: !process.exe_path }
  if (!process.exe_path) {
    void invoke<string>('get_process_path', { pid: process.pid }).then((path) => {
      if (contextMenu.value?.process.pid === process.pid) {
        contextMenu.value.path = path
        contextMenu.value.pathLoading = false
      }
    }).catch(() => {
      if (contextMenu.value?.process.pid === process.pid) contextMenu.value.pathLoading = false
    })
  }
}

async function copyText(value: string) {
  try {
    await navigator.clipboard.writeText(value)
    copyStatus.value = 'Скопировано'
    contextMenu.value = null
    window.setTimeout(() => { copyStatus.value = '' }, 1400)
  } catch {
    copyStatus.value = 'Буфер обмена недоступен'
    contextMenu.value = null
    window.setTimeout(() => { copyStatus.value = '' }, 1800)
  }
}

onMounted(() => {
  updateViewport()
  window.addEventListener('pointerdown', onOutsidePointer)
})
onUnmounted(() => window.removeEventListener('pointerdown', onOutsidePointer))

const filteredProcesses = computed(() => {
  const query = search.value.trim().toLocaleLowerCase()
  if (!query) return metrics.processes
  return metrics.processes.filter((process) =>
    process.name.toLocaleLowerCase().includes(query)
    || String(process.pid).includes(query)
    || (process.exe_path ?? '').toLocaleLowerCase().includes(query),
  )
})

const appGroups = computed<AppGroup[]>(() => {
  const groups = new Map<string, AppGroup>()
  for (const process of filteredProcesses.value) {
    const key = process.name.toLocaleLowerCase()
    let group = groups.get(key)
    if (!group) {
      group = {
        key,
        name: process.name,
        members: [],
        cpu: 0,
        privateBytes: 0,
        workingSet: 0,
        readBps: 0,
        writeBps: 0,
      }
      groups.set(key, group)
    }
    group.members.push(process)
    group.cpu += process.cpu
    group.privateBytes += process.private_bytes
    group.workingSet += process.working_set
    group.readBps += process.io_read_bps
    group.writeBps += process.io_write_bps
  }
  return [...groups.values()].sort(compareGroups)
})

const rows = computed<DisplayRow[]>(() => {
  if (mode.value === 'processes') {
    return [...filteredProcesses.value].sort(compareProcesses).map((process) => ({
      type: 'process',
      key: 'process-' + process.pid,
      process,
    }))
  }
  return appGroups.value.flatMap((group) => {
    const groupRow: DisplayRow = { type: 'app', key: 'app-' + group.key, group }
    if (!expanded.value.includes(group.key)) return [groupRow]
    const children = [...group.members].sort(compareProcesses).map((process): DisplayRow => ({
      type: 'child',
      key: 'child-' + group.key + '-' + process.pid,
      groupKey: group.key,
      process,
    }))
    return [groupRow, ...children]
  })
})

watch([search, mode], () => {
  if (scrollElement.value) scrollElement.value.scrollTop = 0
  scrollTop.value = 0
})

function setSort(key: SortKey) {
  if (sortBy.value === key) {
    descending.value = !descending.value
  } else {
    sortBy.value = key
    descending.value = key !== 'name'
  }
}

function compareGroups(a: AppGroup, b: AppGroup) {
  if (sortBy.value === 'name') return a.name.localeCompare(b.name, 'ru')
  const value = sortValue(a, sortBy.value) - sortValue(b, sortBy.value)
  return (descending.value ? -value : value) || a.name.localeCompare(b.name, 'ru')
}

function compareProcesses(a: ProcSample, b: ProcSample) {
  if (sortBy.value === 'name') {
    return a.name.localeCompare(b.name, 'ru') || a.pid - b.pid
  }
  const value = sortValue(a, sortBy.value) - sortValue(b, sortBy.value)
  return (descending.value ? -value : value) || a.pid - b.pid
}

function sortValue(value: AppGroup | ProcSample, key: SortKey) {
  switch (key) {
    case 'cpu': return value.cpu
    case 'memory': return 'privateBytes' in value ? value.privateBytes : value.private_bytes
    case 'read': return 'readBps' in value ? value.readBps : value.io_read_bps
    case 'write': return 'writeBps' in value ? value.writeBps : value.io_write_bps
    case 'name': return 0
  }
}

function toggleGroup(key: string) {
  expanded.value = expanded.value.includes(key)
    ? expanded.value.filter((item) => item !== key)
    : [...expanded.value, key]
}

function formatBytes(value: number) {
  if (value >= 1024 ** 3) return (value / 1024 ** 3).toFixed(1) + ' ГБ'
  if (value >= 1024 ** 2) return Math.round(value / 1024 ** 2) + ' МБ'
  if (value >= 1024) return Math.round(value / 1024) + ' КБ'
  return value + ' Б'
}

function formatRate(value: number) {
  if (value >= 1024 ** 2) return (value / 1024 ** 2).toFixed(1) + ' МБ/с'
  if (value >= 1024) return Math.round(value / 1024) + ' КБ/с'
  return value + ' Б/с'
}
</script>

<template>
  <div class="processes-page">
    <header class="process-page-heading">
      <div>
        <div class="eyebrow">PEAKCORE / МОНИТОРИНГ СИСТЕМЫ</div>
        <h1>Процессы</h1>
        <p>Потребление ресурсов запущенными приложениями</p>
      </div>
      <div class="process-count"><Activity :size="17" /><strong>{{ metrics.processes.length }}</strong><span>процессов</span></div>
    </header>

    <section class="process-toolbar">
      <div class="mode-switch" role="tablist" aria-label="Режим списка">
        <button :class="{ selected: mode === 'apps' }" @click="mode = 'apps'"><LayoutDashboard :size="16" />Приложения</button>
        <button :class="{ selected: mode === 'processes' }" @click="mode = 'processes'"><Activity :size="16" />Процессы</button>
      </div>
      <label class="process-search">
        <Search :size="17" />
        <input v-model="search" placeholder="Поиск по имени, PID или пути" />
        <button v-if="search" title="Очистить поиск" @click="search = ''"><X :size="15" /></button>
      </label>
    </section>

    <section class="process-table-card">
      <div class="table-summary">
        <span>{{ mode === 'apps' ? appGroups.length + ' приложений' : filteredProcesses.length + ' процессов' }}</span>
        <span v-if="metrics.lastUpdated" class="sample-status"><i></i> Обновлено {{ new Date(metrics.lastUpdated).toLocaleTimeString('ru-RU') }}</span>
        <span v-else class="sample-status waiting"><i></i> Ожидание данных</span>
      </div>
      <div ref="scrollElement" class="process-table-scroll" @scroll="updateViewport">
        <table class="process-table">
          <thead>
            <tr>
              <th class="process-column">
                <button @click="setSort('name')">ИМЯ <component :is="sortBy === 'name' ? (descending ? ArrowDown : ArrowUp) : ChevronDown" :size="13" /></button>
              </th>
              <th><button @click="setSort('cpu')">CPU <component :is="sortBy === 'cpu' ? (descending ? ArrowDown : ArrowUp) : ChevronDown" :size="13" /></button></th>
              <th><button @click="setSort('memory')">PRIVATE <component :is="sortBy === 'memory' ? (descending ? ArrowDown : ArrowUp) : ChevronDown" :size="13" /></button></th>
              <th><span>WORKING SET</span></th>
              <th><button @click="setSort('read')">ЧТЕНИЕ <component :is="sortBy === 'read' ? (descending ? ArrowDown : ArrowUp) : ChevronDown" :size="13" /></button></th>
              <th><button @click="setSort('write')">ЗАПИСЬ <component :is="sortBy === 'write' ? (descending ? ArrowDown : ArrowUp) : ChevronDown" :size="13" /></button></th>
            </tr>
          </thead>
          <tbody v-if="metrics.processes.length">
            <tr v-if="topSpacer" class="virtual-spacer"><td colspan="6" :style="{ height: topSpacer + 'px' }"></td></tr>
            <template v-for="row in visibleRows" :key="row.key">
              <tr v-if="row.type === 'app'" class="app-row" @click="toggleGroup(row.group.key)" @contextmenu="onContextMenu($event, row.group.members[0])">
                <td class="process-column">
                  <div class="process-cell">
                    <button class="expand-button" :aria-label="expanded.includes(row.group.key) ? 'Свернуть группу' : 'Раскрыть группу'"><ChevronDown :size="15" :class="{ rotated: expanded.includes(row.group.key) }" /></button>
                    <span class="process-avatar">{{ row.group.name.slice(0, 1).toUpperCase() }}</span>
                    <span class="process-cell-copy"><strong>{{ row.group.name }}</strong><small>{{ row.group.members.length }} экземпляр(а) · PID {{ row.group.members.map((process) => process.pid).join(', ') }}</small></span>
                  </div>
                </td>
                <td><ResourceBar :value="row.group.cpu" :label="row.group.cpu.toFixed(1) + '%'" kind="cpu" /></td>
                <td class="numeric-cell">{{ formatBytes(row.group.privateBytes) }}</td>
                <td class="numeric-cell muted-cell">{{ formatBytes(row.group.workingSet) }}</td>
                <td class="numeric-cell muted-cell">{{ formatRate(row.group.readBps) }}</td>
                <td class="numeric-cell muted-cell">{{ formatRate(row.group.writeBps) }}</td>
              </tr>
              <tr v-else-if="row.type === 'child'" class="process-row-child" @contextmenu="onContextMenu($event, row.process)">
                <td class="process-column">
                  <div class="process-cell child-cell">
                    <span class="child-branch"></span>
                    <span class="process-avatar child-avatar">{{ row.process.name.slice(0, 1).toUpperCase() }}</span>
                    <span class="process-cell-copy"><strong>{{ row.process.name }}</strong><small>PID {{ row.process.pid }} · {{ row.process.threads }} потоков · {{ row.process.handles }} handles</small></span>
                  </div>
                </td>
                <td><ResourceBar :value="row.process.cpu" :label="row.process.cpu.toFixed(1) + '%'" kind="cpu" /></td>
                <td class="numeric-cell">{{ formatBytes(row.process.private_bytes) }}</td>
                <td class="numeric-cell muted-cell">{{ formatBytes(row.process.working_set) }}</td>
                <td class="numeric-cell muted-cell">{{ formatRate(row.process.io_read_bps) }}</td>
                <td class="numeric-cell muted-cell">{{ formatRate(row.process.io_write_bps) }}</td>
              </tr>
              <tr v-else class="single-process-row" @contextmenu="onContextMenu($event, row.process)">
                <td class="process-column">
                  <div class="process-cell">
                    <span class="process-avatar">{{ row.process.name.slice(0, 1).toUpperCase() }}</span>
                    <span class="process-cell-copy"><strong>{{ row.process.name }}</strong><small>PID {{ row.process.pid }} · {{ row.process.threads }} потоков · {{ row.process.handles }} handles</small></span>
                  </div>
                </td>
                <td><ResourceBar :value="row.process.cpu" :label="row.process.cpu.toFixed(1) + '%'" kind="cpu" /></td>
                <td class="numeric-cell">{{ formatBytes(row.process.private_bytes) }}</td>
                <td class="numeric-cell muted-cell">{{ formatBytes(row.process.working_set) }}</td>
                <td class="numeric-cell muted-cell">{{ formatRate(row.process.io_read_bps) }}</td>
                <td class="numeric-cell muted-cell">{{ formatRate(row.process.io_write_bps) }}</td>
              </tr>
            </template>
            <tr v-if="bottomSpacer" class="virtual-spacer"><td colspan="6" :style="{ height: bottomSpacer + 'px' }"></td></tr>
          </tbody>
        </table>
        <div v-if="!metrics.processes.length" class="empty-processes">
          <span class="empty-icon"><Cpu :size="24" /></span>
          <strong>{{ search ? 'Ничего не найдено' : 'Нет данных о процессах' }}</strong>
          <p>{{ search ? 'Измените поисковый запрос.' : 'Выборка появится, когда окно будет открыто.' }}</p>
        </div>
        <div v-else-if="!rows.length" class="empty-processes compact-empty"><strong>По запросу ничего не найдено</strong></div>
      </div>
    </section>
    <div v-if="contextMenu" class="process-context-menu" :style="{ left: contextMenu.x + 'px', top: contextMenu.y + 'px' }" @pointerdown.stop>
      <strong>{{ contextMenu.process.name }}</strong>
      <button @click="copyText(String(contextMenu.process.pid))">Копировать PID {{ contextMenu.process.pid }}</button>
      <button v-if="contextMenu.path" @click="copyText(contextMenu.path)">Копировать путь к файлу</button>
      <span v-else class="context-unavailable">{{ contextMenu.pathLoading ? 'Получаем путь…' : 'Путь недоступен' }}</span>
    </div>
    <div v-if="copyStatus" class="copy-toast">{{ copyStatus }}</div>
    <footer class="page-footer"><span>Память: Private Bytes · Working Set показан отдельно</span><span>Поиск учитывает имя, PID и путь к файлу</span></footer>
  </div>
</template>

<script lang="ts">
import { defineComponent, h } from 'vue'

const ResourceBar = defineComponent({
  props: {
    value: { type: Number, required: true },
    label: { type: String, required: true },
    kind: { type: String, required: true },
  },
  setup(props) {
    return () => h('div', { class: 'resource-bar-cell' }, [
      h('span', { class: 'resource-number' }, props.label),
      h('i', { class: 'resource-track' }, [
        h('b', { class: 'resource-fill ' + props.kind, style: { width: Math.min(100, Math.max(2, props.value * 10)) + '%' } }),
      ]),
    ])
  },
})

export default defineComponent({ components: { ResourceBar } })
</script>
