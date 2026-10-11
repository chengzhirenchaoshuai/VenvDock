import { createApp } from "vue";
import App from "./App.vue";
import "./style.css";
import { restoreCachedTheme } from "./theme";

// 挂载前先套用上次的主题，避免闪烁
restoreCachedTheme();

createApp(App).mount("#app");
