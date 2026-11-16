import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
export default defineConfig(({ command }) => ({
  define: {
    __BUILD_INFO__: JSON.stringify({
      channel: command === "serve" ? "开发版" : "正式版",
      time: new Date().toISOString(),
    }),
  },
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
}));
