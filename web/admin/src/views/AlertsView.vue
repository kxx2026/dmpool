<template>
  <div class="alerts">
    <n-space vertical :size="24">
      <div class="page-header">
        <h1>Alerts</h1>
        <n-text depth="3">Configure and manage alert rules</n-text>
      </div>

      <!-- Stats -->
      <n-grid :cols="4" :x-gap="16">
        <n-grid-item>
          <n-card>
            <n-statistic label="Total Alerts" :value="stats.total_alerts" />
          </n-card>
        </n-grid-item>
        <n-grid-item>
          <n-card>
            <n-statistic label="Active" :value="stats.active_alerts" />
          </n-card>
        </n-grid-item>
        <n-grid-item>
          <n-card>
            <n-statistic label="Acknowledged" :value="stats.acknowledged_alerts" />
          </n-card>
        </n-grid-item>
        <n-grid-item>
          <n-card>
            <n-statistic label="By Level" :value="Object.keys(stats.alerts_by_level).length" />
          </n-card>
        </n-grid-item>
      </n-grid>

      <!-- Tabs -->
      <n-card :bordered="false">
        <n-tabs v-model:value="activeTab" type="line">
          <n-tab-pane name="rules" tab="Alert Rules">
            <n-space vertical :size="16">
              <n-button type="primary" @click="showAddRuleModal = true">
                <template #icon>
                  <n-icon><AddIcon /></n-icon>
                </template>
                Add Rule
              </n-button>

              <n-data-table
                :columns="ruleColumns"
                :data="rules"
                :loading="loading"
                :row-key="(row) => row.id"
              />
            </n-space>
          </n-tab-pane>

          <n-tab-pane name="history" tab="Alert History">
            <n-data-table
              :columns="alertColumns"
              :data="alerts"
              :loading="loading"
              :pagination="{ pageSize: 20 }"
              :row-key="(row) => row.id"
            />
          </n-tab-pane>

          <n-tab-pane name="channels" tab="Channels">
            <n-empty description="Configure alert channels (Telegram, Webhook) in the server configuration." />
          </n-tab-pane>
        </n-tabs>
      </n-card>
    </n-space>

    <!-- Acknowledge Modal -->
    <n-modal v-model:show="showAckModal" preset="dialog" title="Acknowledge Alert">
      <n-space vertical>
        <n-text v-if="selectedAlert">
          {{ selectedAlert.message }}
        </n-text>
        <n-input
          v-model:value="ackNote"
          type="textarea"
          placeholder="Add a note (optional)"
          :autosize="{ minRows: 3, maxRows: 5 }"
        />
      </n-space>
      <template #action>
        <n-space>
          <n-button @click="showAckModal = false">Cancel</n-button>
          <n-button type="primary" @click="confirmAcknowledge">Acknowledge</n-button>
        </n-space>
      </template>
    </n-modal>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted, h, computed } from 'vue'
import {
  NSpace,
  NCard,
  NGrid,
  NGridItem,
  NStatistic,
  NTabs,
  NTabPane,
  NButton,
  NIcon,
  NDataTable,
  NTag,
  NModal,
  NText,
  NInput,
  NEmpty,
  NSwitch,
  useMessage
} from 'naive-ui'
import {
  AddOutline as AddIcon,
  NotificationsOutline as AlertIcon,
  CheckmarkDoneOutline as AckIcon
} from '@vicons/ionicons5'
import { api, type AlertRule, type Alert, type AlertStats, AlertLevel } from '@/api'

const message = useMessage()

const loading = ref(false)
const activeTab = ref('rules')
const showAckModal = ref(false)
const showAddRuleModal = ref(false)
const ackNote = ref('')

const rules = ref<AlertRule[]>([])
const alerts = ref<Alert[]>([])
const selectedAlert = ref<Alert | null>(null)

const stats = ref<AlertStats>({
  total_alerts: 0,
  active_alerts: 0,
  acknowledged_alerts: 0,
  alerts_by_level: {},
  alerts_by_rule: {}
})

const ruleColumns = [
  { title: 'Name', key: 'name', width: 200 },
  { title: 'Description', key: 'description', ellipsis: { tooltip: true } },
  {
    title: 'Level',
    key: 'level',
    width: 100,
    render: (row: AlertRule) => h(NTag, {
      type: getLevelType(row.level),
      bordered: false
    }, { default: () => row.level.toUpperCase() })
  },
  {
    title: 'Status',
    key: 'enabled',
    width: 100,
    render: (row: AlertRule) => h(NSwitch, {
      value: row.enabled,
      onUpdateValue: (v: boolean) => toggleRule(row.id, v)
    })
  },
  { title: 'Cooldown', key: 'cooldown_minutes', width: 100, render: (row: AlertRule) => `${row.cooldown_minutes}m` }
]

const alertColumns = [
  {
    title: 'Level',
    key: 'level',
    width: 100,
    render: (row: Alert) => h(NTag, {
      type: getLevelType(row.level),
      bordered: false
    }, { default: () => row.level.toUpperCase() })
  },
  { title: 'Title', key: 'title', width: 250 },
  { title: 'Message', key: 'message', ellipsis: { tooltip: true } },
  {
    title: 'Triggered',
    key: 'triggered_at',
    width: 180,
    render: (row: Alert) => new Date(row.triggered_at).toLocaleString()
  },
  {
    title: 'Status',
    key: 'acknowledged',
    width: 120,
    render: (row: Alert) => h(NTag, {
      type: row.acknowledged ? 'default' : 'error',
      bordered: false
    }, { default: () => row.acknowledged ? 'Acknowledged' : 'Active' })
  },
  {
    title: 'Actions',
    key: 'actions',
    width: 100,
    render: (row: Alert) => !row.acknowledged ? h(NButton, {
      size: 'small',
      onClick: () => acknowledgeAlert(row)
    }, {
      icon: () => h(NIcon, null, { default: () => h(AckIcon) }),
      default: () => 'Acknowledge'
    }) : null
  }
]

function getLevelType(level: AlertLevel): 'info' | 'warning' | 'error' | 'success' {
  const types = {
    [AlertLevel.Info]: 'info',
    [AlertLevel.Warning]: 'warning',
    [AlertLevel.Critical]: 'error'
  }
  return types[level] || 'info'
}

async function loadRules() {
  loading.value = true
  try {
    // TODO: Load from API
    rules.value = []
  } catch (error: any) {
    message.error(error.message || 'Failed to load rules')
  } finally {
    loading.value = false
  }
}

async function loadAlerts() {
  loading.value = true
  try {
    // TODO: Load from API
    alerts.value = []
    await loadStats()
  } catch (error: any) {
    message.error(error.message || 'Failed to load alerts')
  } finally {
    loading.value = false
  }
}

async function loadStats() {
  try {
    stats.value = await api.getAlertStats()
  } catch (error: any) {
    console.error('Failed to load alert stats:', error)
  }
}

function toggleRule(id: string, enabled: boolean) {
  // TODO: Implement toggle
  message.info(`Rule ${id} ${enabled ? 'enabled' : 'disabled'}`)
}

function acknowledgeAlert(alert: Alert) {
  selectedAlert.value = alert
  ackNote.value = ''
  showAckModal.value = true
}

async function confirmAcknowledge() {
  if (!selectedAlert.value) return

  try {
    // TODO: Call API
    showAckModal.value = false
    message.success('Alert acknowledged')
    loadAlerts()
  } catch (error: any) {
    message.error(error.message || 'Failed to acknowledge alert')
  }
}

onMounted(() => {
  loadRules()
  loadAlerts()
})
</script>

<style scoped>
.alerts {
  max-width: 1400px;
  margin: 0 auto;
}

.page-header h1 {
  margin: 0;
  font-size: 24px;
  font-weight: 600;
}
</style>
