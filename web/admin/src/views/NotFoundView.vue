<template>
  <div class="not-found">
    <n-result status="404" title="404" description="Page not found">
      <template #footer>
        <n-button @click="goHome">Back to Dashboard</n-button>
      </template>
    </n-result>
  </div>
</template>

<script setup lang="ts">
import { useRouter } from 'vue-router'
import { NResult, NButton } from 'naive-ui'

const router = useRouter()

function goHome() {
  router.push({ name: 'Dashboard' })
}
</script>

<style scoped>
.not-found {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 60vh;
}
</style>
