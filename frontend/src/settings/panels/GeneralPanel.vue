<script setup lang="ts">
import {
  SettingGroup,
  SettingRow,
  SettingSelect,
  SettingSwitch,
} from '../components'
import type { AppSettings } from '../types'

defineProps<{
  state: AppSettings
}>()

const themeOptions = [
  { label: '浅色', value: 'light' },
  { label: '深色', value: 'dark' },
  { label: '跟随系统', value: 'system' },
]

const languageOptions = [
  { label: '简体中文', value: 'zh-CN' },
  { label: 'English', value: 'en-US' },
]

const updateChannelOptions = [
  { label: '正式版', value: 'stable' },
  { label: 'Beta', value: 'beta' },
]

const closeActionOptions = [
  { label: '最小化到托盘', value: 'minimize' },
  { label: '退出程序', value: 'quit' },
]
</script>

<template>
  <SettingGroup
    title="启动"
    description="控制应用随系统启动、关闭主窗口时的行为。"
  >
    <SettingRow
      title="开机时启动"
      description="登录系统后自动在后台运行本应用。"
    >
      <SettingSwitch v-model="state.general.launchAtLogin" aria-label="开机启动" />
    </SettingRow>
    <SettingRow
      title="启动时最小化到托盘"
      description="应用启动时不显示主窗口,仅保留托盘图标。"
    >
      <SettingSwitch v-model="state.general.startMinimized" aria-label="启动时最小化" />
    </SettingRow>
    <SettingRow
      title="显示托盘图标"
      description="关闭后无法通过托盘唤起主窗口。"
    >
      <SettingSwitch v-model="state.general.showTrayIcon" aria-label="显示托盘图标" />
    </SettingRow>
    <SettingRow
      title="点击关闭按钮时"
      description="选择关闭主窗口时的默认行为。"
    >
      <SettingSelect
        v-model="state.general.closeAction"
        :options="closeActionOptions"
      />
    </SettingRow>
  </SettingGroup>

  <SettingGroup title="外观" description="主题与界面语言,深色模式后续将持续完善。">

  <SettingGroup
    title="窗口策略"
    description="翻译弹窗与截图 overlay 的预创建策略，重启应用后生效。"
  >
    <SettingRow
      title="预创建翻译弹窗"
      description="应用启动时即创建翻译窗口，划词时响应更快。"
    >
      <SettingSwitch v-model="state.general.popupPrecreate" aria-label="预创建翻译弹窗" />
    </SettingRow>
    <SettingRow
      title="预创建截图 Overlay"
      description="应用启动时即创建截图 overlay 窗口，截图 OCR 时响应更快。"
    >
      <SettingSwitch v-model="state.general.overlayPrecreate" aria-label="预创建截图 Overlay" />
    </SettingRow>
  </SettingGroup>
    <SettingRow
      title="主题"
      description="决定主窗口与设置页面的色彩风格。深色模式尚在开发,当前默认浅色。"
      status="wip"
    >
      <SettingSelect v-model="state.general.theme" :options="themeOptions" />
    </SettingRow>
    <SettingRow title="界面语言" description="重启后生效。">
      <SettingSelect v-model="state.general.language" :options="languageOptions" />
    </SettingRow>
  </SettingGroup>

  <SettingGroup title="更新" description="有新版本时的获取渠道与检测行为。">
    <SettingRow
      title="更新通道"
      description="Beta 通道会获得更早的功能,但稳定性可能不足。"
    >
      <SettingSelect v-model="state.general.updateChannel" :options="updateChannelOptions" />
    </SettingRow>
    <SettingRow
      title="自动检测更新"
      description="每隔 24 小时检查一次。"
      status="wip"
    >
      <SettingSwitch v-model="state.general.autoCheckUpdate" aria-label="自动检测更新" />
    </SettingRow>
  </SettingGroup>
</template>
