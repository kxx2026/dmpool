<template>
  <div class="dashboard">
    <n-space vertical :size="24">
      <!-- Page Header -->
      <div class="page-header">
        <h1>Dashboard</h1>
        <n-text depth="3">Real-time pool metrics and status</n-text>
      </div>

      <!-- Metrics Cards -->
      <n-grid :cols="4" :x-gap="16" :y-gap="16" responsive="screen">
        <n-grid-item>
          <n-card class="metric-card">
            <n-statistic label="Pool Hashrate" :value="formatHashrate(metrics.pool_hashrate_ths)">
              <template #prefix>
                <n-icon color="#18a058"><HashrateIcon /></n-icon>
              </template>
            </n-statistic>
          </n-card>
        </n-grid-item>

        <n-grid-item>
          <n-card class="metric-card">
            <n-statistic label="Active Workers" :value="metrics.active_workers">
              <template #prefix>
                <n-icon color="#2080f0"><WorkersIcon /></n-icon>
              </template>
            </n-statistic>
          </n-card>
        </n-grid-item>

        <n-grid-item>
          <n-card class="metric-card">
            <n-statistic label="Total Shares" :value="metrics.total_shares">
              <template #prefix>
                <n-icon color="#f0a020"><SharesIcon /></n-icon>
              </template>
            </n-statistic>
          </n-card>
        </n-grid-item>

        <n-grid-item>
          <n-card class="metric-card">
            <n-statistic label="Blocks Found" :value="metrics.blocks_found">
              <template #prefix>
                <n-icon color="#d03050"><BlocksIcon /></n-icon>
              </template>
            </n-statistic>
          </n-card>
        </n-grid-item>
      </n-grid>

      <!-- Charts Row -->
      <n-grid :cols="2" :x-gap="16" responsive="screen">
        <n-grid-item>
          <n-card title="Hashrate History" :bordered="false">
            <div ref="hashrateChartRef" style="height: 300px;"></div>
          </n-card>
        </n-grid-item>

        <n-grid-item>
          <n-card title="Shares Distribution" :bordered="false">
            <div ref="sharesChartRef" style="height: 300px;"></div>
          </n-card>
        </n-grid-item>
      </n-grid>

      <!-- Additional Info -->
      <n-grid :cols="3" :x-gap="16" responsive="screen">
        <n-grid-item>
          <n-card title="Pool Status" :bordered="false">
            <n-space vertical>
              <n-statistic label="Uptime" :value="formatUptime(metrics.uptime_seconds)" />
              <n-statistic label="PPLNS Window" :value="metrics.pplns_window_shares" suffix="shares" />
              <n-statistic label="Current Difficulty" :value="metrics.current_difficulty.toFixed(2)" />
            </n-space>
          </n-card>
        </n-grid-item>

        <n-grid-item :span="2">
          <n-card title="Recent Activity" :bordered="false">
            <n-list>
              <n-list-item v-for="activity in recentActivities" :key="activity.id">
                <template #prefix>
                  <n-icon :color="activity.color">
                    <ActivityIcon :type="activity.type" />
                  </n-icon>
                </template>
                <n-text>{{ activity.message }}</n-text>
                <template #suffix>
                  <n-text depth="3" style="font-size: 12px;">{{ activity.time }}</n-text>
                </template>
              </n-list-item>
            </n-list>
          </n-card>
        </n-grid-item>
      </n-grid>
    </n-space>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, h } from 'vue'
import {
  NSpace,
  NCard,
  NGrid,
  NGridItem,
  NStatistic,
  NIcon,
  NText,
  NList,
  NListItem,
  useMessage,
  type MessageReactive
} from 'naive-ui'
import * as echarts from 'echarts'
import type { EChartsOption } from 'echarts'
import VChart from 'vue-echarts'
import {
  PulseOutline as HashrateIcon,
  PeopleOutline as WorkersIcon,
  GridOutline as SharesIcon,
  CubeOutline as BlocksIcon,
  CheckmarkCircle as SuccessIcon,
  WarningOutline as WarningIcon,
  CloseCircle as ErrorIcon,
  InformationCircle as InfoIcon
} from '@vicons/ionicons5'
import { api, type DashboardMetrics } from '@/api'

const message = useMessage()
const loadingMsg = ref<MessageReactive | null>(null)
const hashrateChartRef = ref<HTMLElement>()
const sharesChartRef = ref<HTMLElement>()

let hashrateChart: echarts.ECharts | null = null
let sharesChart: echarts.ECharts | null = null
let refreshInterval: number | null = null

const metrics = ref<DashboardMetrics>({
  pool_hashrate_ths: 0,
  active_workers: 0,
  total_shares: 0,
  blocks_found: 0,
  uptime_seconds: 0,
  pplns_window_shares: 0,
  current_difficulty: 0
})

const recentActivities = ref([
  {
    id: 1,
    type: 'success',
    color: '#18a058',
    message: 'New block found at height 12345',
    time: '5 min ago'
  },
  {
    id: 2,
    type: 'warning',
    color: '#f0a020',
    message: 'Worker bc1q... disconnected',
    time: '15 min ago'
  },
  {
    id: 3,
    type: 'info',
    color: '#2080f0',
    message: 'Configuration reloaded',
    time: '1 hour ago'
  }
])

const ActivityIcon = (props: { type: string }) => {
  const icons = {
    success: SuccessIcon,
    warning: WarningIcon,
    error: ErrorIcon,
    info: InfoIcon
  }
  return h(icons[props.type as keyof typeof icons] || InfoIcon)
}

function formatHashrate(ths: number): string {
  if (ths >= 1000) {
    return `${(ths / 1000).toFixed(2)} PH/s`
  }
  return `${ths.toFixed(2)} TH/s`
}

function formatUptime(seconds: number): string {
  const days = Math.floor(seconds / 86400)
  const hours = Math.floor((seconds % 86400) / 3600)
  const minutes = Math.floor((seconds % 3600) / 60)

  if (days > 0) {
    return `${days}d ${hours}h`
  } else if (hours > 0) {
    return `${hours}h ${minutes}m`
  }
  return `${minutes}m`
}

function initCharts() {
  if (hashrateChartRef.value) {
    hashrateChart = echarts.init(hashrateChartRef.value)
    updateHashrateChart()
  }

  if (sharesChartRef.value) {
    sharesChart = echarts.init(sharesChartRef.value)
    updateSharesChart()
  }
}

function updateHashrateChart() {
  if (!hashrateChart) return

  const option: EChartsOption = {
    grid: { top: 10, right: 10, bottom: 20, left: 40 },
    xAxis: {
      type: 'category',
      data: ['00:00', '04:00', '08:00', '12:00', '16:00', '20:00', '24:00']
    },
    yAxis: { type: 'value', name: 'TH/s' },
    series: [{
      data: [120, 132, 101, 134, 90, 230, 210],
      type: 'line',
      smooth: true,
      areaStyle: {
        color: {
          type: 'linear',
          x: 0, y: 0, x2: 0, y2: 1,
          colorStops: [
            { offset: 0, color: 'rgba(24, 160, 88, 0.3)' },
            { offset: 1, color: 'rgba(24, 160, 88, 0)' }
          ]
        }
      },
      itemStyle: { color: '#18a058' }
    }],
    tooltip: { trigger: 'axis' }
  }

  hashrateChart.setOption(option)
}

function updateSharesChart() {
  if (!sharesChart) return

  const option: EChartsOption = {
    tooltip: { trigger: 'item' },
    series: [{
      type: 'pie',
      radius: ['40%', '70%'],
      data: [
        { value: 1048, name: 'Valid Shares' },
        { value: 735, name: 'Stale Shares' },
        { value: 580, name: 'Invalid Shares' }
      ],
      emphasis: {
        itemStyle: {
          shadowBlur: 10,
          shadowOffsetX: 0,
          shadowColor: 'rgba(0, 0, 0, 0.5)'
        }
      }
    }]
  }

  sharesChart.setOption(option)
}

async function loadDashboard() {
  try {
    const data = await api.getDashboard()
    metrics.value = data
  } catch (error: any) {
    console.error('Failed to load dashboard:', error)
  }
}

function startAutoRefresh() {
  refreshInterval = window.setInterval(() => {
    loadDashboard()
  }, 30000) // Refresh every 30 seconds
}

onMounted(() => {
  loadDashboard()
  initCharts()
  startAutoRefresh()
})

onUnmounted(() => {
  if (hashrateChart) hashrateChart.dispose()
  if (sharesChart) sharesChart.dispose()
  if (refreshInterval) clearInterval(refreshInterval)
})
</script>

<style scoped>
.dashboard {
  max-width: 1400px;
  margin: 0 auto;
}

.page-header h1 {
  margin: 0;
  font-size: 24px;
  font-weight: 600;
}

.metric-card {
  background: white;
}

.metric-card :deep(.n-statistic .n-statistic__label) {
  font-size: 12px;
  font-weight: 500;
}

.metric-card :deep(.n-statistic .n-statistic__value) {
  font-size: 24px;
  font-weight: 600;
}
</style>
