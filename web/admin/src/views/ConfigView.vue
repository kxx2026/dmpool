<template>
  <div class="config">
    <n-space vertical :size="24">
      <!-- Page Header -->
      <div class="page-header">
        <h1>Configuration</h1>
        <n-text depth="3">Manage pool settings</n-text>
      </div>

      <!-- Warning Banner -->
      <n-alert type="warning" :bordered="false">
        Some configuration changes require confirmation before being applied. Dangerous changes are marked with warning icons.
      </n-alert>

      <!-- Config Sections -->
      <n-grid :cols="2" :x-gap="16" responsive="screen">
        <!-- Stratum Configuration -->
        <n-grid-item :span="2">
          <n-card title="Stratum Settings">
            <n-form ref="stratumFormRef" :model="stratumConfig" label-placement="left" label-width="200px">
              <n-grid :cols="2" :x-gap="16">
                <n-grid-item>
                  <n-form-item label="Port" path="stratum_port">
                    <n-input-number v-model:value="stratumConfig.stratum_port" :min="1" :max="65535" />
                  </n-form-item>
                </n-grid-item>
                <n-grid-item>
                  <n-form-item label="Hostname" path="stratum_hostname">
                    <n-input v-model:value="stratumConfig.stratum_hostname" />
                  </n-form-item>
                </n-grid-item>
                <n-grid-item>
                  <n-form-item label="Start Difficulty" path="start_difficulty">
                    <n-input-number v-model:value="stratumConfig.start_difficulty" :min="8" :max="512" />
                  </n-form-item>
                </n-grid-item>
                <n-grid-item>
                  <n-form-item label="Minimum Difficulty" path="minimum_difficulty">
                    <n-input-number v-model:value="stratumConfig.minimum_difficulty" :min="8" :max="512" />
                  </n-form-item>
                </n-grid-item>
              </n-grid>
            </n-form>
          </n-card>
        </n-grid-item>

        <!-- PPLNS Configuration -->
        <n-grid-item>
          <n-card title="PPLNS Settings">
            <n-space vertical :size="16">
              <n-form-item label="TTL (Days)">
                <n-input-number v-model:value="pplnsConfig.pplns_ttl_days" :min="1" :max="30" />
              </n-form-item>
              <n-form-item label="Difficulty Multiplier">
                <n-input-number v-model:value="pplnsConfig.difficulty_multiplier" :min="0.1" :max="10" :step="0.1" />
              </n-form-item>
              <n-alert type="warning" size="small">
                TTL less than 7 days may cause miners to lose earnings. TTL of 0 will prevent payments.
              </n-alert>
            </n-space>
          </n-card>
        </n-grid-item>

        <!-- Pool Settings -->
        <n-grid-item>
          <n-card title="Pool Settings">
            <n-space vertical :size="16">
              <n-form-item label="Donation">
                <n-input-number v-model:value="poolConfig.donation" :min="0" :max="10000" />
                <template #feedback>
                  {{ (poolConfig.donation / 100).toFixed(2) }}%
                </template>
              </n-form-item>
              <n-form-item label="Ignore Difficulty">
                <n-switch v-model:value="poolConfig.ignore_difficulty" />
              </n-form-item>
              <n-form-item label="Pool Signature">
                <n-input v-model:value="poolConfig.pool_signature" placeholder="Optional" maxlength="16" />
              </n-form-item>
              <n-alert type="error" size="small" v-if="poolConfig.donation === 10000">
                Donation set to 100% - miners will receive zero payouts!
              </n-alert>
            </n-space>
          </n-card>
        </n-grid-item>
      </n-grid>

      <!-- Actions -->
      <n-space justify="end">
        <n-button @click="loadConfig">
          <template #icon>
            <n-icon><RefreshIcon /></n-icon>
          </template>
          Reload
        </n-button>
        <n-button type="primary" @click="handleUpdateConfig" :loading="updating">
          <template #icon>
            <n-icon><SaveIcon /></n-icon>
          </template>
          Save Changes
        </n-button>
      </n-space>
    </n-space>

    <!-- Pending Confirmations -->
    <n-modal v-model:show="showConfirmations" preset="card" title="Pending Confirmations" style="width: 600px;">
      <n-space vertical :size="12">
        <n-text v-if="pendingConfirmations.length === 0">
          No pending confirmations.
        </n-text>
        <n-list v-else>
          <n-list-item v-for="conf in pendingConfirmations" :key="conf.id">
            <n-space vertical :size="8">
              <n-space justify="space-between">
                <n-text strong>{{ conf.parameter }}</n-text>
                <n-tag :type="getRiskType(conf.risk_level)" size="small">
                  {{ conf.risk_level }}
                </n-tag>
              </n-space>
              <n-text depth="3">{{ conf.risk_description }}</n-text>
              <n-code>{{ JSON.stringify(conf.new_value) }}</n-code>
              <n-space>
                <n-button size="small" type="success" @click="confirmChange(conf.id)">
                  Confirm
                </n-button>
                <n-button size="small" @click="cancelChange(conf.id)">
                  Cancel
                </n-button>
              </n-space>
            </n-space>
          </n-list-item>
        </n-list>
      </n-space>
    </n-modal>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted } from 'vue'
import {
  NSpace,
  NCard,
  NGrid,
  NGridItem,
  NForm,
  NFormItem,
  NInput,
  NInputNumber,
  NSwitch,
  NButton,
  NIcon,
  NAlert,
  NModal,
  NText,
  NTag,
  NList,
  NListItem,
  NCode,
  useMessage,
  useDialog
} from 'naive-ui'
import {
  RefreshOutline as RefreshIcon,
  SaveOutline as SaveIcon
} from '@vicons/ionicons5'
import { api, type ConfigView } from '@/api'

const message = useMessage()
const dialog = useDialog()

const updating = ref(false)
const showConfirmations = ref(false)

const stratumConfig = reactive({
  stratum_port: 3333,
  stratum_hostname: '',
  start_difficulty: 32,
  minimum_difficulty: 16
})

const pplnsConfig = reactive({
  pplns_ttl_days: 7,
  difficulty_multiplier: 1.0
})

const poolConfig = reactive({
  donation: 0,
  ignore_difficulty: false,
  pool_signature: ''
})

const pendingConfirmations = ref<any[]>([])

async function loadConfig() {
  try {
    const config = await api.getConfig()
    Object.assign(stratumConfig, {
      stratum_port: config.stratum_port,
      stratum_hostname: config.stratum_hostname,
      start_difficulty: config.start_difficulty,
      minimum_difficulty: config.minimum_difficulty
    })
    Object.assign(pplnsConfig, {
      pplns_ttl_days: config.pplns_ttl_days,
      difficulty_multiplier: config.difficulty_multiplier
    })
    Object.assign(poolConfig, {
      donation: config.donation,
      ignore_difficulty: config.ignore_difficulty,
      pool_signature: config.pool_signature || ''
    })
    message.success('Configuration loaded')
  } catch (error: any) {
    message.error(error.message || 'Failed to load configuration')
  }
}

async function handleUpdateConfig() {
  updating.value = true
  try {
    const update = {
      ...stratumConfig,
      ...pplnsConfig,
      ...poolConfig
    }

    await api.updateConfig(update)
    message.success('Configuration updated. Please confirm pending changes.')
    showConfirmations.value = true
    loadPendingConfirmations()
  } catch (error: any) {
    message.error(error.message || 'Failed to update configuration')
  } finally {
    updating.value = false
  }
}

async function loadPendingConfirmations() {
  // TODO: Load from API
  pendingConfirmations.value = []
}

async function confirmChange(id: string) {
  try {
    // TODO: Call API to confirm
    message.success('Change confirmed')
    loadPendingConfirmations()
  } catch (error: any) {
    message.error(error.message || 'Failed to confirm change')
  }
}

async function cancelChange(id: string) {
  try {
    // TODO: Call API to cancel
    message.success('Change cancelled')
    loadPendingConfirmations()
  } catch (error: any) {
    message.error(error.message || 'Failed to cancel change')
  }
}

function getRiskType(level: string): 'success' | 'warning' | 'error' | 'info' {
  const types: Record<string, 'success' | 'warning' | 'error' | 'info'> = {
    'Safe': 'success',
    'Low': 'info',
    'Medium': 'warning',
    'High': 'warning',
    'Critical': 'error'
  }
  return types[level] || 'info'
}

onMounted(() => {
  loadConfig()
})
</script>

<style scoped>
.config {
  max-width: 1400px;
  margin: 0 auto;
}

.page-header h1 {
  margin: 0;
  font-size: 24px;
  font-weight: 600;
}
</style>
