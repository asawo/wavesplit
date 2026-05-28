import "@fontsource/oleo-script-swash-caps";
// @ts-ignore — font CSS import, no type declarations
import "@fontsource-variable/material-symbols-rounded";
import { mount } from "svelte";
import App from "./App.svelte";

const app = mount(App, {
  target: document.getElementById("app"),
});

export default app;
