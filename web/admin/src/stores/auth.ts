// Authentication Store
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { api, type LoginRequest, type UserInfo } from '@/api'
import { useMessage } from 'naive-ui'

export const useAuthStore = defineStore('auth', () => {
  const user = ref<UserInfo | null>(null)
  const loading = ref(false)
  const message = useMessage()

  const isAuthenticated = computed(() => !!api.isAuthenticated() && !!user.value)
  const isAdmin = computed(() => user.value?.role === 'admin')

  async function login(credentials: LoginRequest) {
    loading.value = true
    try {
      const result = await api.login(credentials)
      user.value = result.user
      message.success('Login successful')
      return true
    } catch (error: any) {
      message.error(error.message || 'Login failed')
      return false
    } finally {
      loading.value = false
    }
  }

  function logout() {
    user.value = null
    api.logout()
    message.info('Logged out')
  }

  function init() {
    if (api.isAuthenticated()) {
      // Token exists in localStorage, assume user is logged in
      // In production, you might want to verify the token with the server
      user.value = {
        username: 'admin', // This should come from the token
        role: 'admin'
      }
    }
  }

  return {
    user,
    loading,
    isAuthenticated,
    isAdmin,
    login,
    logout,
    init
  }
})
