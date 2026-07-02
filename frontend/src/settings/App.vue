<script setup lang="ts">
import { ref, onMounted, computed } from 'vue';
import type { AppConfig, Provider } from '@/types/config';
import { validateConfig } from '@/lib/config';
import { invokeGetAppConfig, invokeSaveAppConfig, isTauriReady } from '@/lib/tauri';
import TargetLangSection from './components/TargetLangSection.vue';
import ProviderSelect from './components/ProviderSelect.vue';
import OpenAiSection from './components/OpenAiSection.vue';
import ClaudeSection from './components/ClaudeSection.vue';
import StrategySection from './components/StrategySection.vue';
import SaveBar from './components/SaveBar.vue';

const config = ref<AppConfig | null>(null);
const status = ref('');
const isError = ref(false);
const saving = ref(false);

const showOpenAi = computed(() => config.value?.provider === 'openai-compatible');
const showClaude = computed(() => config.value?.provider === 'claude');

function setStatus(msg: string, err = false) {
  status.value = msg;
  isError.value = err;
}

async function load() {
  if (!isTauriReady()) {
    setStatus('Tauri API 未就绪，无法读取配置', true);
    return;
  }
  try {
    config.value = await invokeGetAppConfig();
    setStatus('');
  } catch (e) {
    setStatus(String(e), true);
  }
}

async function save() {
  if (!config.value) return;
  if (!isTauriReady()) {
    setStatus('Tauri API 未就绪，无法保存配置', true);
    return;
  }
  const err = validateConfig(config.value);
  if (err) {
    setStatus(err, true);
    return;
  }
  saving.value = true;
  setStatus('保存中...');
  const before = { popup: config.value.popupPrecreate, overlay: config.value.overlayPrecreate };
  try {
    const saved = await invokeSaveAppConfig(config.value);
    config.value = saved;
    const changed = saved.popupPrecreate !== before.popup || saved.overlayPrecreate !== before.overlay;
    setStatus(changed ? '配置已保存，窗口策略切换需重启应用生效' : '配置已保存，下一次翻译生效');
  } catch (e) {
    setStatus(String(e), true);
  } finally {
    saving.value = false;
  }
}

function setProvider(p: Provider) {
  if (config.value) config.value = { ...config.value, provider: p };
}

onMounted(load);
</script>

<template>
  <div v-if="config" class="mx-auto max-w-2xl space-y-6 p-6">
    <header class="space-y-1">
      <h1 class="text-2xl font-semibold">Shizi</h1>
      <p class="text-muted-foreground">设置</p>
    </header>

    <TargetLangSection v-model="config.targetLang" />

    <ProviderSelect :model-value="config.provider" @update:model-value="setProvider" />

    <OpenAiSection v-if="showOpenAi" v-model="config.openaiCompatible" />
    <ClaudeSection v-if="showClaude" v-model="config.claude" />

    <StrategySection
      v-model:popup-precreate="config.popupPrecreate"
      v-model:overlay-precreate="config.overlayPrecreate"
      v-model:collect-usage="config.collectUsage"
    />

    <SaveBar :saving="saving" :status="status" :is-error="isError" @save="save" />
  </div>
  <div v-else class="p-6 text-muted-foreground">
    {{ status || '加载中...' }}
  </div>
</template>
