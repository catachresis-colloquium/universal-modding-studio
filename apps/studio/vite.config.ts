// SPDX-License-Identifier: AGPL-3.0-or-later
import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

export default defineConfig({
  plugins: [vue()],
  server: {
    host: "127.0.0.1",
    proxy: { "/api": "http://127.0.0.1:4178" }
  }
});
