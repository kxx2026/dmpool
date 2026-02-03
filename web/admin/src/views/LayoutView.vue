<template>
  <n-layout has-sider style="height: 100vh">
    <n-layout-sider
      bordered
      collapse-mode="width"
      :collapsed-width="64"
      :width="240"
      :collapsed="collapsed"
      show-trigger
      @collapse="collapsed = true"
      @expand="collapsed = false"
    >
      <div class="logo">
        <n-icon size="32" color="#18a058">
          <LogoIcon />
        </n-icon>
        <span v-if="!collapsed" class="logo-text">DMPool</span>
      </div>

      <n-menu
        :collapsed="collapsed"
        :collapsed-width="64"
        :collapsed-icon-size="22"
        :options="menuOptions"
        :value="activeKey"
        @update:value="handleMenuSelect"
      />
    </n-layout-sider>

    <n-layout>
      <n-layout-header bordered class="header">
        <div class="header-content">
          <n-breadcrumb>
            <n-breadcrumb-item>{{ currentMenuLabel }}</n-breadcrumb-item>
          </n-breadcrumb>

          <div class="header-actions">
            <n-space>
              <n-tag :type="healthStatusType" size="small">
                {{ healthStatus }}
              </n-tag>

              <n-dropdown :options="userMenuOptions" @select="handleUserMenuSelect">
                <n-button circle quaternary>
                  <template #icon>
                    <n-icon><UserIcon /></n-icon>
                  </template>
                </n-button>
              </n-dropdown>
            </n-space>
          </div>
        </div>
      </n-layout-header>

      <n-layout-content content-style="padding: 24px;" class="content">
        <router-view />
      </n-layout-content>
    </n-layout>
  </n-layout>
</template>

<script setup lang="ts">
import { ref, computed, h } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import {
  NLayout,
  NLayoutSider,
  NLayoutHeader,
  NLayoutContent,
  NMenu,
  NBreadcrumb,
  NBreadcrumbItem,
  NSpace,
  NTag,
  NButton,
  NDropdown,
  NIcon,
  type MenuOption
} from 'naive-ui'
import {
  DashboardOutline as DashboardIcon,
  PeopleOutline as WorkersIcon,
  SettingsOutline as ConfigIcon,
  DocumentTextOutline as AuditIcon,
  CloudBackupOutline as BackupIcon,
  NotificationsOutline as AlertsIcon,
  LogOutOutline as LogoutIcon,
  PersonOutline as UserIcon
} from '@vicons/ionicons5'

const LogoIcon = () => h('svg', { viewBox: '0 0 24 24' }, [
  h('path', {
    fill: 'currentColor',
    d: 'M12 2L2 7l10 5 10-5-10-5zm0 9l2.5-1.25L12 8.5l-2.5 1.25L12 11zm0 2.5l-5-2.5-5 2.5L12 22l10-8.5-5-2.5-5 2.5z'
  })
])

const router = useRouter()
const route = useRoute()

const collapsed = ref(false)
const healthStatus = ref('Healthy')
const healthStatusType = ref<'success' | 'warning' | 'error'>('success')

const menuOptions: MenuOption[] = [
  {
    label: 'Dashboard',
    key: 'Dashboard',
    icon: () => h(NIcon, null, { default: () => h(DashboardIcon) })
  },
  {
    label: 'Workers',
    key: 'Workers',
    icon: () => h(NIcon, null, { default: () => h(WorkersIcon) })
  },
  {
    label: 'Configuration',
    key: 'Config',
    icon: () => h(NIcon, null, { default: () => h(ConfigIcon) })
  },
  {
    label: 'Audit Logs',
    key: 'Audit',
    icon: () => h(NIcon, null, { default: () => h(AuditIcon) })
  },
  {
    label: 'Backup',
    key: 'Backup',
    icon: () => h(NIcon, null, { default: () => h(BackupIcon) })
  },
  {
    label: 'Alerts',
    key: 'Alerts',
    icon: () => h(NIcon, null, { default: () => h(AlertsIcon) })
  }
]

const activeKey = computed(() => route.name as string)
const currentMenuLabel = computed(() => {
  const option = menuOptions.find(o => o.key === activeKey.value)
  return option?.label || 'Dashboard'
})

const userMenuOptions = [
  {
    label: 'Logout',
    key: 'logout',
    icon: () => h(NIcon, null, { default: () => h(LogoutIcon) })
  }
]

function handleMenuSelect(key: string) {
  router.push({ name: key })
}

function handleUserMenuSelect(key: string) {
  if (key === 'logout') {
    const { useAuthStore } = require('@/stores/auth')
    const authStore = useAuthStore()
    authStore.logout()
    router.push({ name: 'Login' })
  }
}
</script>

<style scoped>
.logo {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 20px;
  font-size: 20px;
  font-weight: bold;
  color: #18a058;
}

.logo-text {
  white-space: nowrap;
}

.header {
  height: 64px;
  padding: 0 24px;
  display: flex;
  align-items: center;
}

.header-content {
  display: flex;
  justify-content: space-between;
  align-items: center;
  width: 100%;
}

.content {
  background: #f5f7fa;
  min-height: calc(100vh - 64px);
}
</style>
