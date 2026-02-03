<template>
  <div class="backup">
    <n-space vertical :size="24">
      <div class="page-header">
        <h1>Backup & Restore</h1>
        <n-text depth="3">Database backup management</n-text>
      </div>

      <!-- Quick Actions -->
      <n-card :bordered="false">
        <n-space :size="16">
          <n-button type="primary" @click="createBackup" :loading="creating">
            <template #icon>
              <n-icon><AddIcon /></n-icon>
            </template>
            Create Backup
          </n-button>
          <n-button @click="loadBackups">
            <template #icon>
              <n-icon><RefreshIcon /></n-icon>
            </template>
            Refresh
          </n-button>
          <n-button @click="cleanupBackups" type="warning">
            <template #icon>
              <n-icon><TrashIcon /></n-icon>
            </template>
            Cleanup Old
          </n-button>
        </n-space>
      </n-card>

      <!-- Stats -->
      <n-grid :cols="4" :x-gap="16">
        <n-grid-item>
          <n-card>
            <n-statistic label="Total Backups" :value="stats.total_backups" />
          </n-card>
        </n-grid-item>
        <n-grid-item>
          <n-card>
            <n-statistic label="Total Size" :value="formatBytes(stats.total_size_bytes)" />
          </n-card>
        </n-grid-item>
        <n-grid-item>
          <n-card>
            <n-statistic label="Disk Usage" :value="formatBytes(stats.disk_usage_bytes)" />
          </n-card>
        </n-grid-item>
        <n-grid-item>
          <n-card>
            <n-statistic label="Latest Backup" :value="formatDate(stats.latest_backup)" />
          </n-card>
        </n-grid-item>
      </n-grid>

      <!-- Backups Table -->
      <n-card title="Available Backups" :bordered="false">
        <n-data-table
          :columns="columns"
          :data="backups"
          :loading="loading"
          :pagination="{ pageSize: 10 }"
          :row-key="(row) => row.id"
        />
      </n-card>
    </n-space>

    <!-- Restore Confirmation -->
    <n-modal v-model:show="showRestoreModal" preset="dialog" title="Restore Backup">
      <n-space vertical>
        <n-alert type="warning">
          Restoring a backup will replace the current database. The service will need to be restarted after restoration.
        </n-alert>
        <n-text v-if="selectedBackup">
          Backup: {{ selectedBackup.file_path }}<br>
          Size: {{ formatBytes(selectedBackup.backup_size) }}<br>
          Created: {{ formatDate(selectedBackup.timestamp) }}
        </n-text>
      </n-space>
      <template #action>
        <n-space>
          <n-button @click="showRestoreModal = false">Cancel</n-button>
          <n-button type="error" @click="confirmRestore" :loading="restoring">
            Restore
          </n-button>
        </n-space>
      </template>
    </n-modal>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, h } from 'vue'
import {
  NSpace,
  NCard,
  NGrid,
  NGridItem,
  NStatistic,
  NButton,
  NIcon,
  NDataTable,
  NTag,
  NModal,
  NAlert,
  NText,
  useMessage,
  useDialog
} from 'naive-ui'
import {
  AddOutline as AddIcon,
  RefreshOutline as RefreshIcon,
  TrashOutline as TrashIcon,
  DownloadOutline as DownloadIcon,
  CloudUploadOutline as RestoreIcon,
  Trash as DeleteIcon
} from '@vicons/ionicons5'
import { api, type BackupMetadata, type BackupStats } from '@/api'

const message = useMessage()
const dialog = useDialog()

const loading = ref(false)
const creating = ref(false)
const restoring = ref(false)
const showRestoreModal = ref(false)

const backups = ref<BackupMetadata[]>([])
const stats = ref<BackupStats>({
  total_backups: 0,
  total_size_bytes: 0,
  disk_usage_bytes: 0
})

const selectedBackup = ref<BackupMetadata | null>(null)

const columns = [
  {
    title: 'ID',
    key: 'id',
    width: 100,
    render: (row: BackupMetadata) => row.id.slice(0, 8) + '...'
  },
  {
    title: 'Created',
    key: 'timestamp',
    width: 180,
    render: (row: BackupMetadata) => new Date(row.timestamp).toLocaleString()
  },
  {
    title: 'Original Size',
    key: 'original_size',
    width: 120,
    render: (row: BackupMetadata) => formatBytes(row.original_size)
  },
  {
    title: 'Backup Size',
    key: 'backup_size',
    width: 120,
    render: (row: BackupMetadata) => formatBytes(row.backup_size)
  },
  {
    title: 'Compression',
    key: 'compression_ratio',
    width: 100,
    render: (row: BackupMetadata) => row.compression_ratio ? `${row.compression_ratio.toFixed(1)}%` : '-'
  },
  {
    title: 'Status',
    key: 'validated',
    width: 100,
    render: (row: BackupMetadata) => h(NTag, {
      type: row.validated ? 'success' : 'warning',
      bordered: false
    }, { default: () => row.validated ? 'Valid' : 'Pending' })
  },
  {
    title: 'Actions',
    key: 'actions',
    width: 120,
    render: (row: BackupMetadata) => h(NSpace, { size: 8 },
    [
      h(NButton, {
        size: 'small',
        quaternary: true,
        onClick: () => openRestoreModal(row)
      }, {
        icon: () => h(NIcon, null, { default: () => h(RestoreIcon) })
      }),
      h(NButton, {
        size: 'small',
        quaternary: true,
        type: 'error',
        onClick: () => deleteBackup(row)
      }, {
        icon: () => h(NIcon, null, { default: () => h(DeleteIcon) })
      })
    ]
  )
  }
]

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B'
  const k = 1024
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return `${(bytes / Math.pow(k, i)).toFixed(2)} ${sizes[i]}`
}

function formatDate(dateStr?: string): string {
  if (!dateStr) return 'Never'
  return new Date(dateStr).toLocaleDateString()
}

async function loadBackups() {
  loading.value = true
  try {
    const result = await api.listBackups()
    backups.value = result.backups
  } catch (error: any) {
    message.error(error.message || 'Failed to load backups')
  } finally {
    loading.value = false
  }

  try {
    stats.value = await api.getBackupStats()
  } catch (error: any) {
    console.error('Failed to load backup stats:', error)
  }
}

async function createBackup() {
  creating.value = true
  try {
    await api.createBackup()
    message.success('Backup created successfully')
    loadBackups()
  } catch (error: any) {
    message.error(error.message || 'Failed to create backup')
  } finally {
    creating.value = false
  }
}

function openRestoreModal(backup: BackupMetadata) {
  selectedBackup.value = backup
  showRestoreModal.value = true
}

async function confirmRestore() {
  if (!selectedBackup.value) return

  restoring.value = true
  try {
    await api.restoreBackup(selectedBackup.value.id)
    message.success('Backup restored. Please restart the service.')
    showRestoreModal.value = false
  } catch (error: any) {
    message.error(error.message || 'Failed to restore backup')
  } finally {
    restoring.value = false
  }
}

function deleteBackup(backup: BackupMetadata) {
  dialog.warning({
    title: 'Delete Backup',
    content: `Are you sure you want to delete backup ${backup.id.slice(0, 8)}...?`,
    positiveText: 'Delete',
    negativeText: 'Cancel',
    onPositiveClick: async () => {
      try {
        await api.deleteBackup(backup.id)
        message.success('Backup deleted')
        loadBackups()
      } catch (error: any) {
        message.error(error.message || 'Failed to delete backup')
      }
    }
  })
}

async function cleanupBackups() {
  dialog.info({
    title: 'Cleanup Old Backups',
    content: 'This will delete old backups based on the retention policy (7 days). Continue?',
    positiveText: 'Cleanup',
    negativeText: 'Cancel',
    onPositiveClick: async () => {
      try {
        await api.cleanupBackups()
        message.success('Old backups cleaned up')
        loadBackups()
      } catch (error: any) {
        message.error(error.message || 'Failed to cleanup backups')
      }
    }
  })
}

onMounted(() => {
  loadBackups()
})
</script>

<style scoped>
.backup {
  max-width: 1400px;
  margin: 0 auto;
}

.page-header h1 {
  margin: 0;
  font-size: 24px;
  font-weight: 600;
}
</style>
