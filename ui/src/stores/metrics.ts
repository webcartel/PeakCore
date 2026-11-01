import { defineStore } from 'pinia'

export interface SysSnapshot {
  ts_ms: number
  cpu_total: number
  cpu_cores: number[]
  config_revision: number
  mem: {
    total: number
    used: number
    available: number
    cached: number
    commit_used: number
    commit_limit: number
    pagefile_used: number
    pagefile_total: number
  }
}

export interface ProcSample {
  pid: number
  ppid: number
  name: string
  exe_path: string | null
  cpu: number
  private_bytes: number
  working_set: number
  io_read_bps: number
  io_write_bps: number
  threads: number
  handles: number
  gpu_pct: number
  gpu_memory: number
}

export interface GpuProcessMetrics {
  pid: number
  utilization_pct: number
  memory: number
}

export interface HistoryPoint {
  ts_ms: number
  cpu: number
  mem_pct: number
}

export interface MinuteAggregate {
  minute_ts_ms: number
  cpu_avg: number
  cpu_max: number
  mem_avg: number
  mem_max: number
  samples: number
}

export interface NetworkInterfaceMetrics {
  id: number
  index: number
  name: string
  rx_bps: number
  tx_bps: number
}

export interface GpuEngineMetrics {
  name: string
  utilization_pct: number
}

export interface ResourceMetrics {
  ts_ms: number
  disk_available: boolean
  disk_busy_available: boolean
  disk_read_bps: number
  disk_write_bps: number
  disk_busy_pct: number
  network_rx_bps: number
  network_tx_bps: number
  network_available: boolean
  interfaces: NetworkInterfaceMetrics[]
  gpu_available: boolean
  gpu_utilization_pct: number
  gpu_memory_used: number
  gpu_engines: GpuEngineMetrics[]
  gpu_processes: GpuProcessMetrics[]
}

export interface DashboardUpdate {
  ts_ms: number
  snapshot: SysSnapshot | null
  history: HistoryPoint[]
  aggregates: MinuteAggregate[]
  processes: ProcSample[]
  resources: ResourceMetrics | null
  daemon_connected: boolean
  error: string | null
}

export const useMetricsStore = defineStore('metrics', {
  state: () => ({
    snapshot: null as SysSnapshot | null,
    history: [] as HistoryPoint[],
    aggregates: [] as MinuteAggregate[],
    processes: [] as ProcSample[],
    resources: null as ResourceMetrics | null,
    daemonConnected: false,
    error: null as string | null,
    lastUpdated: 0,
  }),
  actions: {
    apply(update: DashboardUpdate) {
      this.snapshot = update.snapshot
      this.history = update.history
      this.aggregates = update.aggregates ?? []
      const gpu = new Map(
        (update.resources?.gpu_processes ?? []).map((entry) => [entry.pid, entry]),
      )
      this.processes = update.processes.map((process) => {
        const usage = gpu.get(process.pid)
        return {
          ...process,
          gpu_pct: usage?.utilization_pct ?? 0,
          gpu_memory: usage?.memory ?? 0,
        }
      })
      this.resources = update.resources
      this.daemonConnected = update.daemon_connected
      this.error = update.error
      this.lastUpdated = update.ts_ms
    },
    replaceHistory(points: HistoryPoint[]) {
      this.history = points
    },
  },
})
