/**
 * 本地 vendored 品牌 logo（来源：@lobehub/icons-static-svg color 变体）。
 * simple-icons 没有的国内厂商用此表补齐；有道 / 讯飞 lobe 暂无收录，继续 Lucide 兜底。
 *
 * 路径：frontend/src/assets/service-icons/
 * slug 对照：zhipu、siliconcloud（硅基流动）、volcengine、tencent
 */
import type { ServiceId } from './types'

const zhipu = new URL('../assets/service-icons/zhipu.svg', import.meta.url).href
const siliconcloud = new URL('../assets/service-icons/siliconcloud.svg', import.meta.url).href
const volcengine = new URL('../assets/service-icons/volcengine.svg', import.meta.url).href
const tencent = new URL('../assets/service-icons/tencent.svg', import.meta.url).href

const SERVICE_LOGO_SRC: Partial<Record<ServiceId, string>> = {
  zhipu,
  siliconflow: siliconcloud,
  volcengine,
  tencent,
}

export const getServiceLogoSrc = (id: ServiceId): string | undefined => SERVICE_LOGO_SRC[id]
