<template>
  <div class="audit">
    <n-space vertical :size="24">
      <div class="page-header">
        <h1>Audit Logs</h1>
        <n-text depth="3">Security and compliance audit trail</n-text>
      </div>

      <!-- Stats Cards -->
      <n-grid :cols="4" :x-gap="16">
        <n-grid-item>
          <n-card>
            <n-statistic label="Total Logs" :value="stats.total_logs" />
          </n-card>
        </n-grid-item>
        <n-grid-item>
          <n-card>
            <n-statistic label="Successful" :value="stats.success_count" />
          </n-card>
        </n-grid-item>
        <n-grid-item>
          <n-card>
            <n-statistic label="Failed" :value="stats.failure_count" />
          </n-card>
        </n-grid-item>
        <n-grid-item>
          <n-card>
            <n-statistic label="Today's Logs" :value="todayLogsCount" />
          </n-card>
        </n-grid-item>
      </n-grid>

      <!-- Filters -->
      <n-card :bordered="false">
        <n-space :size="16">
          <n-input
            v-model:value="filter.username"
            placeholder="Filter by username"
            clearable
            style="width: 150px;"
            @update:value="loadLogs"
          />
          <n-input
            v-model:value="filter.action"
            placeholder="Filter by action"
            clearable
            style="width: 150px;"
            @update:value="loadLogs"
          />
          <n-select
            v-model:value="timeRange"
            :options="timeRangeOptions"
            style="width: 150px;"
            @update:value="loadLogs"
          />
        </n-space>
      </n-card>

      <!-- Logs Table -->
      <n-card :bordered="false">
        <n-data-table
          :columns="columns"
          :data="logs"
          :loading="loading"
          :pagination="paginationConfig"
          :row-key="(row) => row.id"
        />
      </n-card>
    </n-space>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, computed, onMounted, h } from 'vue'
import {
  NSpace,
  NCard,
  NGrid,
  NGridItem,
  NStatistic,
  NInput,
  NSelect,
  NButton,
  NDataTable,
  NTag,
  NIcon,
  useMessage
} from 'naive-ui'
import {
  CheckmarkCircleOutline as SuccessIcon,
  CloseCircleOutline as ErrorIcon
} from '@vicons/ionicons5'
import { api, type AuditLog, type AuditStats, type AuditFilter } from '@/api'

const message = useMessage()

const loading = ref(false)
const logs = ref<AuditLog[]>([])
const stats = ref<AuditStats>({
  total_logs: 0,
  success_count: 0,
  failure_count: 0,
  top_actions: []
})

const filter = reactive<AuditFilter>({
  limit: 100
})

const timeRange = ref('24h')

const timeRangeOptions = [
  { label: 'Last Hour', value: '1h' },
  { label: 'Last 24 Hours', value: '24h' },
  { label: 'Last 7 Days', value: '7d' },
  { label: 'All Time', value: 'all' }
]

const todayLogsCount = computed(() => {
  const today = new Date().toDateString()
  return logs.value.filter(log => new Date(log.timestamp).toDateString() === today).length
})

const columns = [
  { title: 'Timestamp', key: 'timestamp', width: 180, render: (row: AuditLog) => new Date(row.timestamp).toLocaleString() },
  { title: 'User', key: 'username', width: 120 },
  { title: 'Action', key: 'action', width: 150 },
  { title: 'Resource', key: 'resource', width: 200, ellipsis: { tooltip: true } },
  {
    title: 'Status',
    key: 'success',
    width: 100,
    render: (row: AuditLog) => h(NTag, {
      type: row.success ? 'success' : 'error',
      bordered: false
    }, {
      icon: () => h(NIcon, null, { default: () => row.success ? SuccessIcon : ErrorIcon }),
      default: () => row.success ? 'Success' : 'Failed'
    })
  },
  { title: 'IP Address', key: 'ip_address', width: 140 },
  {
    title: 'Details',
    key: 'details',
    width: 100,
    render: (row: AuditLog) => h(NButton, {
      text: true,
      onClick: () => showDetails(row)
    }, { default: () => 'View' })
  }
]

const paginationConfig = reactive({
  pageSize: 50
})

async function loadLogs() {
  loading.value = true
  try {
    const endTime = Date.now() / 1000
    let startTime: number | undefined

    switch (timeRange.value) {
      case '1h':
        startTime = endTime - 3600
        break
      case '24h':
        startTime = endTime - 86400
        break
      case '7d':
        startTime = endTime - 604800
        break
    }

    const params: AuditFilter = {
      ...filter,
      start_time: startTime,
      end_time: endTime
    }

    logs.value = await api.getAuditLogs(params)
    await loadStats()
  } catch (error: any) {
    message.error(error.message || 'Failed to load audit logs')
  } finally {
    loading.value = false
  }
}

async function loadStats() {
  try {
    stats.value = await api.getAuditStats()
  } catch (error: any) {
    console.error('Failed to load audit stats:', error)
  }
}

function showDetails(log: AuditLog) {
  message.info(JSON.stringify(log.details, null, 2), { duration: 5000 })
}

onMounted(() => {
  loadLogs()
})
</script>

<style scoped>
.audit {
  max-width: 1400px;
  margin: 0 auto;
}

.page-header h1 {
  margin: 0;
  font-size: 24px;
  font-weight: 600;
}
</style>
