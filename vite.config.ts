import { resolve } from "node:path";
import { defineConfig } from "vite";

// Локальные окна приложения (сайт платформы открывается напрямую, без сборки).
export default defineConfig({
  clearScreen: false,
  server: { port: 1420, strictPort: true, watch: { ignored: ["**/src-tauri/**"] } },
  build: {
    target: "es2022",
    rollupOptions: {
      input: {
        settings: resolve(__dirname, "settings.html"),
        peer: resolve(__dirname, "peer.html"),
        friends: resolve(__dirname, "friends.html"),
      },
    },
  },
});
