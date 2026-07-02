<script setup lang="ts">
import { TooltipProvider } from 'reka-ui'
import SettingsPage from '@/settings/SettingsPage.vue'
import { Toaster } from '@/components/ui/toast'
</script>

<template>
  <TooltipProvider :delay-duration="250" :skip-delay-duration="300">
    <SettingsPage />
  </TooltipProvider>
  <Toaster />
</template>
