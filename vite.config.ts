import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { fileURLToPath, URL } from "node:url";
import { defineConfig } from "vite";

const host = process.env.TAURI_DEV_HOST;

function getNodeModulePackageName(id: string) {
  const modulePath = id.split("node_modules/")[1];
  if (!modulePath) return null;

  const [firstPart, secondPart] = modulePath.split(/[\\/]/);
  return firstPart.startsWith("@") ? `${firstPart}/${secondPart}` : firstPart;
}

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
  build: {
    rollupOptions: {
      output: {
        manualChunks(id) {
          const packageName = getNodeModulePackageName(id);
          if (!packageName) return undefined;

          if (["react", "react-dom", "scheduler"].includes(packageName)) {
            return "react-vendor";
          }

          if (packageName.startsWith("@tauri-apps/")) {
            return "tauri-vendor";
          }

          if (
            [
              "@base-ui/react",
              "cmdk",
              "i18next",
              "lucide-react",
              "react-i18next",
            ].includes(packageName)
          ) {
            return "ui-vendor";
          }

          return "vendor";
        },
      },
    },
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
