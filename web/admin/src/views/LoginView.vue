<template>
  <div class="login-container">
    <n-card class="login-card" title="DMPool Admin">
      <n-form ref="formRef" :model="form" :rules="rules" size="large">
        <n-form-item path="username" label="Username">
          <n-input
            v-model:value="form.username"
            placeholder="Enter your username"
            @keydown.enter="handleLogin"
          />
        </n-form-item>

        <n-form-item path="password" label="Password">
          <n-input
            v-model:value="form.password"
            type="password"
            show-password-on="click"
            placeholder="Enter your password"
            @keydown.enter="handleLogin"
          />
        </n-form-item>

        <n-form-item>
          <n-button
            type="primary"
            block
            size="large"
            :loading="loading"
            @click="handleLogin"
          >
            Login
          </n-button>
        </n-form-item>
      </n-form>
    </n-card>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { NCard, NForm, NFormItem, NInput, NButton, type FormInst, type FormRules } from 'naive-ui'
import { useAuthStore } from '@/stores/auth'

const router = useRouter()
const route = useRoute()
const authStore = useAuthStore()

const formRef = ref<FormInst | null>(null)
const loading = ref(false)

const form = reactive({
  username: '',
  password: ''
})

const rules: FormRules = {
  username: {
    required: true,
    message: 'Please enter your username',
    trigger: ['blur', 'input']
  },
  password: {
    required: true,
    message: 'Please enter your password',
    trigger: ['blur', 'input']
  }
}

async function handleLogin() {
  if (!formRef.value) return

  try {
    await formRef.value.validate()
    loading.value = true

    const success = await authStore.login({
      username: form.username,
      password: form.password
    })

    if (success) {
      const redirect = (route.query.redirect as string) || '/'
      router.push(redirect)
    }
  } catch (e) {
    // Validation failed
  } finally {
    loading.value = false
  }
}
</script>

<style scoped>
.login-container {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 100vh;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
}

.login-card {
  width: 100%;
  max-width: 400px;
  box-shadow: 0 10px 40px rgba(0, 0, 0, 0.2);
}
</style>
