<template>
  <div class="workers">
    <n-space vertical :size="24">
      <!-- Page Header -->
      <div class="page-header">
        <h1>Workers</h1>
        <n-text depth="3">Manage pool workers and miners</n-text>
      </div>

      <!-- Filters -->
      <n-card :bordered="false">
        <n-space :size="16">
          <n-input
            v-model:value="searchQuery"
            placeholder="Search by address or worker name..."
            clearable
            @update:value="handleSearch"
            style="width: 300px;"
          >
            <template #prefix>
              <n-icon><SearchIcon /></n-icon>
            </template>
          </n-input>

          <n-select
            v-model:value="statusFilter"
            placeholder="Filter by status"
            clearable
            :options="statusOptions"
            style="width: 150px;"
            @update:value="handleFilterChange"
          />

          <n-select
            v-model:value="sortBy"
            :options="sortOptions"
            style="width: 150px;"
            @update:value="handleSortChange"
          />

          <n-button
            quaternary
            @click="toggleSortOrder"
          >
            <template #icon>
              <n-icon>
                <SortAscendingIcon v-if="sortOrder === 'asc'" />
                <SortDescendingIcon v-else />
              </n-icon>
            </template>
          </n-button>
        </n-space>
      </n-card>

      <!-- Workers Table -->
      <n-card :bordered="false">
        <n-data-table
          :columns="columns"
          :data="workers"
          :loading="loading"
          :pagination="paginationConfig"
          :row-key="(row: WorkerInfo) => row.address"
        />
      </n-card>
    </n-space>

    <!-- Ban Dialog -->
    <n-modal v-model:show="showBanDialog" preset="dialog" title="Ban Worker">
      <n-space vertical>
        <n-text>Are you sure you want to ban this worker?</n-text>
        <n-input
          v-model:value="banReason"
          type="textarea"
          placeholder="Reason for banning (optional)"
          :autosize="{ minRows: 3, maxRows: 5 }"
        />
      </n-space>
      <template #action>
        <n-space>
          <n-button @click="showBanDialog = false">Cancel</n-button>
          <n-button type="error" @click="confirmBan">Ban Worker</n-button>
        </n-space>
      </template>
    </n-modal>

    <!-- Tags Dialog -->
    <n-modal v-model:show="showTagsDialog" preset="dialog" title="Manage Tags">
      <n-space vertical>
        <n-space>
          <n-tag
            v-for="tag in currentWorkerTags"
            :key="tag"
            closable
            @close="removeTag(tag)"
          >
            {{ tag }}
          </n-tag>
          <n-tag v-if="currentWorkerTags.length === 0" :bordered="false">
            No tags
          </n-tag>
        </n-space>
        <n-input
          v-model:value="newTag"
          placeholder="Add new tag..."
          @keyup.enter="addTag"
        >
          <template #suffix>
            <n-button text @click="addTag">Add</n-button>
          </template>
        </n-input>
      </n-space>
      <template #action>
        <n-button @click="showTagsDialog = false">Close</n-button>
      </template>
    </n-modal>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted, h } from 'vue'
import {
  NSpace,
  NCard,
  NInput,
  NSelect,
  NButton,
  NIcon,
  NDataTable,
  NModal,
  NTag,
  NText,
  useMessage,
  useDialog,
  type DataTableColumns
} from 'naive-ui'
import {
  SearchOutline as SearchIcon,
  SortAscendingOutline as SortAscendingIcon,
  SortDescendingOutline as SortDescendingIcon,
  BanOutline as BanIcon,
  PricetagsOutline as TagsIcon,
  CheckmarkCircle as ActiveIcon,
  AlertCircle as InactiveIcon,
  CloseCircle as BannedIcon
} from '@vicons/ionicons5'
import { api, type WorkerInfo, type WorkerPaginationParams } from '@/api'

const message = useMessage()
const dialog = useDialog()

const loading = ref(false)
const workers = ref<WorkerInfo[]>([])
const totalCount = ref(0)

// Filters
const searchQuery = ref('')
const statusFilter = ref<string | null>(null)
const sortBy = ref('last_seen')
const sortOrder = ref<'asc' | 'desc'>('desc')

// Pagination
const currentPage = ref(1)
const pageSize = ref(20)

// Dialogs
const showBanDialog = ref(false)
const showTagsDialog = ref(false)
const banReason = ref('')
const selectedWorker = ref<WorkerInfo | null>(null)
const newTag = ref('')
const currentWorkerTags = ref<string[]>([])

const statusOptions = [
  { label: 'Active', value: 'active' },
  { label: 'Inactive', value: 'inactive' },
  { label: 'Banned', value: 'banned' }
]

const sortOptions = [
  { label: 'Last Seen', value: 'last_seen' },
  { label: 'Hashrate', value: 'hashrate' },
  { label: 'Shares', value: 'shares' },
  { label: 'Address', value: 'address' }
]

const columns: DataTableColumns<WorkerInfo> = [
  {
    title: 'Address',
    key: 'address',
    width: 200,
    ellipsis: { tooltip: true }
  },
  {
    title: 'Worker',
    key: 'worker_name',
    width: 150
  },
  {
    title: 'Hashrate',
    key: 'hashrate_ths',
    width: 120,
    render: (row) => `${row.hashrate_ths.toFixed(2)} TH/s`
  },
  {
    title: 'Shares',
    key: 'shares_count',
    width: 100
  },
  {
    title: 'Difficulty',
    key: 'difficulty',
    width: 100
  },
  {
    title: 'Status',
    key: 'status',
    width: 100,
    render: (row) => {
      const config = {
        active: { type: 'success', icon: ActiveIcon },
        inactive: { type: 'warning', icon: InactiveIcon },
        banned: { type: 'error', icon: BannedIcon }
      }[row.status]

      return h(NTag, { type: config.type, bordered: false }, {
        default: () => row.status.toUpperCase()
      })
    }
  },
  {
    title: 'Tags',
    key: 'tags',
    width: 200,
    render: (row) => h('div', { class: 'tags-cell' },
      row.tags.map(tag =>
        h(NTag, { size: 'small', type: 'info' }, { default: () => tag })
      )
    )
  },
  {
    title: 'Last Seen',
    key: 'last_seen',
    width: 180,
    render: (row) => new Date(row.last_seen).toLocaleString()
  },
  {
    title: 'Actions',
    key: 'actions',
    width: 150,
    fixed: 'right',
    render: (row) => h(NSpace, { size: 8 },
      [
        h(NButton, {
          size: 'small',
          quaternary: true,
          onClick: () => openTagsDialog(row)
        }, {
          icon: () => h(NIcon, null, { default: () => h(TagsIcon) })
        }),
        h(NButton, {
          size: 'small',
          quaternary: true,
          type: row.is_banned ? 'success' : 'error',
          onClick: () => row.is_banned ? unbanWorker(row) : openBanDialog(row)
        }, {
          icon: () => h(NIcon, null, { default: () => h(BanIcon) }),
          default: () => row.is_banned ? 'Unban' : 'Ban'
        })
      ]
    )
  }
]

const paginationConfig = reactive({
  page: currentPage,
  pageSize: pageSize,
  showSizePicker: true,
  pageSizes: [10, 20, 50, 100],
  onChange: (page: number) => {
    currentPage.value = page
    loadWorkers()
  },
  onUpdatePageSize: (size: number) => {
    pageSize.value = size
    loadWorkers()
  }
})

async function loadWorkers() {
  loading.value = true
  try {
    const params: WorkerPaginationParams = {
      page: currentPage.value,
      page_size: pageSize.value,
      search: searchQuery.value || undefined,
      status: statusFilter.value || undefined,
      sort_by: sortBy.value,
      sort_order: sortOrder.value
    }

    const result = await api.getWorkers(params)
    workers.value = result.data
    totalCount.value = result.total
  } catch (error: any) {
    message.error(error.message || 'Failed to load workers')
  } finally {
    loading.value = false
  }
}

function handleSearch() {
  currentPage.value = 1
  loadWorkers()
}

function handleFilterChange() {
  currentPage.value = 1
  loadWorkers()
}

function handleSortChange() {
  loadWorkers()
}

function toggleSortOrder() {
  sortOrder.value = sortOrder.value === 'asc' ? 'desc' : 'asc'
  loadWorkers()
}

function openBanDialog(worker: WorkerInfo) {
  selectedWorker.value = worker
  banReason.value = ''
  showBanDialog.value = true
}

async function confirmBan() {
  if (!selectedWorker.value) return

  try {
    await api.banWorker(selectedWorker.value.address, banReason.value)
    message.success(`Worker ${selectedWorker.value.address} banned`)
    showBanDialog.value = false
    loadWorkers()
  } catch (error: any) {
    message.error(error.message || 'Failed to ban worker')
  }
}

async function unbanWorker(worker: WorkerInfo) {
  dialog.warning({
    title: 'Unban Worker',
    content: `Are you sure you want to unban ${worker.address}?`,
    positiveText: 'Unban',
    negativeText: 'Cancel',
    onPositiveClick: async () => {
      try {
        await api.unbanWorker(worker.address)
        message.success(`Worker ${worker.address} unbanned`)
        loadWorkers()
      } catch (error: any) {
        message.error(error.message || 'Failed to unban worker')
      }
    }
  })
}

function openTagsDialog(worker: WorkerInfo) {
  selectedWorker.value = worker
  currentWorkerTags.value = [...worker.tags]
  newTag.value = ''
  showTagsDialog.value = true
}

async function addTag() {
  if (!newTag.value.trim() || !selectedWorker.value) return

  try {
    await api.addWorkerTag(selectedWorker.value.address, newTag.value.trim())
    currentWorkerTags.value.push(newTag.value.trim())
    newTag.value = ''
    loadWorkers()
  } catch (error: any) {
    message.error(error.message || 'Failed to add tag')
  }
}

async function removeTag(tag: string) {
  if (!selectedWorker.value) return

  try {
    await api.removeWorkerTag(selectedWorker.value.address, tag)
    currentWorkerTags.value = currentWorkerTags.value.filter(t => t !== tag)
    loadWorkers()
  } catch (error: any) {
    message.error(error.message || 'Failed to remove tag')
  }
}

onMounted(() => {
  loadWorkers()
})
</script>

<style scoped>
.workers {
  max-width: 1400px;
  margin: 0 auto;
}

.page-header h1 {
  margin: 0;
  font-size: 24px;
  font-weight: 600;
}

.tags-cell {
  display: flex;
  gap: 4px;
  flex-wrap: wrap;
}
</style>
