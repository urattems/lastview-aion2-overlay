import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

const src = (f: string) => fileURLToPath(new URL(`./src/${f}`, import.meta.url));

// Une page par fenêtre : la bulle et la fiche (les paramètres viendront à la section 8).
export default defineConfig({
  root: "src",
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: {
    outDir: "../dist",
    emptyOutDir: true,
    target: "es2022",
    rollupOptions: {
      input: { bulle: src("bulle.html"), fiche: src("fiche.html") },
    },
  },
});
