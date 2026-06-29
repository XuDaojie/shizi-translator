# Shizi

基于大语言模型的 Windows 桌面翻译助手，灵感来自 macOS 的 Bob。

## 命令

### 运行（开发模式）
```bash
npm run tauri dev
```

### 编译（debug）
```bash
cd src-tauri && cargo build
```

### 打包（release）
```bash
cd src-tauri && cargo build --release
# release exe: src-tauri/target/release/shizi.exe
```

### 生成安装包（MSI/NSIS）
```bash
npm run tauri build
# 安装包: src-tauri/target/release/bundle/msi/ 或 bundle/nshsis/
```

### 调试
```bash
npm run tauri dev
# 或直接运行 release exe:
./src-tauri/target/release/shizi.exe
```

### 清理编译缓存
```bash
cd src-tauri && cargo clean
# 删除整个 target 目录（可节省数 GB 空间）
```

> `npx tauri dev` 也可代替 `npm run tauri dev` 执行。
