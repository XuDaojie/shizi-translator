<script setup lang="ts">
import { computed } from 'vue'
import { useSettings } from './stores/settings'
import SettingsSidebar from './SettingsSidebar.vue'

interface Props {
  /** 侧栏高亮（可先于内容切换，用于导航动画）。 */
  active: string
  /**
   * 主内容区实际挂载的分类。重面板（历史/服务）可晚于侧栏切换，
   * 布局 containment 以内容为准，避免「侧栏已切、外层滚动模式已变、内容仍是上一页」时抢动画帧。
   */
  contentActive?: string
}

const props = withDefaults(defineProps<Props>(), {
  contentActive: undefined,
})

const { state } = useSettings()

const emit = defineEmits<{
  (e: 'update:active', value: string): void
}>()

/**
 * 自管分栏滚动的面板：服务页、翻译历史。
 * 外层不再出滚动条，由面板内左右（或主从）区域各自 overflow。
 * 以 contentActive 为准（缺省回退 active）。
 */
const panelKey = computed(() => props.contentActive ?? props.active)
const isContainedPanel = computed(
  () => panelKey.value === 'services' || panelKey.value === 'history',
)
</script>

<template>
  <div class="flex h-full min-h-0 bg-background">
    <SettingsSidebar
      :model-value="active"
      @update:model-value="(v) => emit('update:active', v)"
    />

    <main class="flex min-h-0 min-w-0 flex-1 flex-col bg-background">
      <div
        :class="[
          'min-h-0 flex-1 p-2.5 scrollbar-thin',
          // 稳定预留纵向滚动条槽，避免长短内容切换时 scrollbar 显隐导致整栏横向抖动
          isContainedPanel ? 'flex flex-col overflow-hidden' : 'overflow-y-auto [scrollbar-gutter:stable]',
        ]"
      >
        <div
          :class="[
            'mx-auto flex max-w-[var(--content-max-width)] flex-col',
            isContainedPanel
              ? 'h-full min-h-0 w-full flex-1 overflow-hidden'
              : 'min-h-full gap-3',
          ]"
        >
          <slot :state="state" />
        </div>
      </div>
    </main>
  </div>
</template>
