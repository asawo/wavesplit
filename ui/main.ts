import "@fontsource/oleo-script-swash-caps";
import "@fontsource-variable/material-symbols-rounded";
import { mount } from "svelte";
import App from "./App.svelte";

const app = mount(App, {
  target: document.getElementById("app")!,
});

export default app;
