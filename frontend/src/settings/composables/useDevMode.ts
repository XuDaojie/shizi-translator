/**
 * 集中环境判据：dev 可见 / release 隐藏的开发中功能统一读此 composable。
 *
 * `import.meta.env.DEV` 是 Vite 编译期常量：
 * - `npm run tauri dev` / `npm run dev` / 走 localhost:5173 的 dev 模式 exe -> `true`
 * - `npm run tauri build`（`vite build` 生产构建 + 打包）-> `false`（被静态替换为字面量）
 *
 * 封装为 composable 便于测试 mock 与未来扩展（如运行时调试开关）。
 */
export function useDevMode(): boolean {
  return import.meta.env.DEV
}
