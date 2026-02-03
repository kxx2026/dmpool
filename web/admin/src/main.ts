// Main Entry Point
import { createApp } from 'vue'
import { createPinia } from 'pinia'
import App from './App.vue'
import router from './router'
import { useDialog, useMessage, useNotification, useLoadingBar } from 'naive-ui'

const app = createApp(App)
const pinia = createPinia()

app.use(pinia)
app.use(router)

// Naive UI providers
app.config.globalProperties.$message = useMessage()
app.config.globalProperties.$dialog = useDialog()
app.config.globalProperties.$notification = useNotification()
app.config.globalProperties.$loadingBar = useLoadingBar()

// Initialize auth store
import { useAuthStore } from '@/stores/auth'
const authStore = useAuthStore()
authStore.init()

app.mount('#app')
