<script setup lang="ts">
import type { PlaygroundTheme } from '../types/playground'

defineProps<{
  duration: number | null
  statusLabel: string
  statusTone: Record<string, boolean>
  theme: PlaygroundTheme
  version: string
}>()

const emit = defineEmits<{
  toggleTheme: []
}>()

function handleToggleTheme(): void {
  emit('toggleTheme')
}
</script>

<template>
  <section class="topbar" aria-labelledby="title">
    <div class="brand">
      <img
        class="brand-logo"
        src="../assets/logo.svg"
        alt=""
        width="1024"
        height="1024"
        aria-hidden="true"
      />
      <div class="brand-copy">
        <p class="eyebrow">oxjsdoc playground (wasm) v{{ version }}</p>
        <h1 id="title" class="product-logo-text">JSDoc AST Explorer</h1>
        <p class="tagline">High performance jsdoc parser</p>
      </div>
    </div>

    <div class="top-actions">
      <a
        class="github-link"
        href="https://github.com/kazupon/ox-jsdoc"
        target="_blank"
        rel="noreferrer"
        aria-label="Open oxjsdoc on GitHub"
      >
        <svg aria-hidden="true" viewBox="0 0 16 16">
          <path
            fill="currentColor"
            d="M8 0C3.58 0 0 3.67 0 8.2c0 3.63 2.29 6.7 5.47 7.79.4.08.55-.18.55-.4 0-.2-.01-.86-.01-1.56-2.01.38-2.53-.5-2.69-.97-.09-.24-.48-.97-.82-1.17-.28-.16-.68-.55-.01-.56.63-.01 1.08.59 1.23.84.72 1.24 1.87.89 2.33.68.07-.53.28-.89.51-1.09-1.78-.21-3.64-.91-3.64-4.04 0-.89.31-1.62.82-2.19-.08-.21-.36-1.04.08-2.16 0 0 .67-.22 2.2.84A7.42 7.42 0 0 1 8 3.94c.68 0 1.36.09 2 .28 1.53-1.06 2.2-.84 2.2-.84.44 1.12.16 1.95.08 2.16.51.57.82 1.3.82 2.19 0 3.14-1.87 3.83-3.65 4.04.29.26.54.75.54 1.52 0 1.09-.01 1.97-.01 2.24 0 .22.15.48.55.4A8.09 8.09 0 0 0 16 8.2C16 3.67 12.42 0 8 0Z"
          />
        </svg>
        GitHub
      </a>
      <button
        type="button"
        class="theme-toggle"
        :aria-pressed="theme === 'dark'"
        @click="handleToggleTheme"
      >
        <span class="toggle-track">
          <span class="toggle-thumb" />
        </span>
        {{ theme === 'light' ? 'Light' : 'Dark' }}
      </button>

      <div class="status" :class="statusTone">
        <span>{{ statusLabel }}</span>
        <strong v-if="duration !== null">{{ duration.toFixed(2) }} ms</strong>
      </div>
    </div>
  </section>
</template>

<style scoped>
.topbar {
  display: flex;
  align-items: end;
  justify-content: space-between;
  gap: 20px;
  padding: 22px 24px;
  border: 1px solid var(--line);
  border-radius: 26px;
  background: var(--panel);
  box-shadow: 0 20px 60px rgba(35, 29, 20, 0.1);
}

.top-actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 10px;
}

.brand {
  display: flex;
  align-items: center;
  min-width: 0;
  gap: 18px;
}

.brand-logo {
  flex: 0 0 auto;
  width: clamp(64px, 8vw, 108px);
  height: clamp(64px, 8vw, 108px);
  filter: drop-shadow(0 16px 28px rgba(8, 145, 178, 0.18));
}

.brand-copy {
  min-width: 0;
}

.eyebrow,
.status {
  font: 700 12px/1 var(--sans);
  letter-spacing: 0.12em;
  text-transform: uppercase;
}

.eyebrow {
  margin: 0 0 10px;
  color: var(--accent);
}

.github-link {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  min-height: 38px;
  padding: 8px 12px;
  border: 1px solid var(--line);
  border-radius: 999px;
  background: var(--panel-strong);
  color: var(--muted);
  font: 700 11px/1 var(--sans);
  letter-spacing: 0.1em;
  text-decoration: none;
  text-transform: uppercase;
}

.github-link:hover {
  border-color: var(--accent);
  color: var(--accent);
}

.github-link svg {
  width: 15px;
  height: 15px;
}

h1 {
  margin: 0;
  font-size: clamp(32px, 5vw, 58px);
  line-height: 0.96;
  letter-spacing: -0.055em;
  white-space: nowrap;
}

.product-logo-text {
  font-family: 'Montserrat', 'Arial Black', 'Helvetica Neue', sans-serif;
  font-weight: 900;
  letter-spacing: -0.04em;
  text-transform: uppercase;
}

.tagline {
  margin: 14px 0 0;
  color: var(--muted);
  font: 700 14px/1.2 var(--sans);
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.status {
  display: inline-flex;
  align-items: center;
  gap: 12px;
  padding: 12px 14px;
  border-radius: 999px;
  background: var(--panel-strong);
  color: var(--muted);
  white-space: nowrap;
}

.theme-toggle {
  display: inline-flex;
  align-items: center;
  gap: 10px;
  min-height: 38px;
  padding: 8px 12px;
  border: 1px solid var(--line);
  border-radius: 999px;
  background: var(--panel-strong);
  color: var(--ink);
  font: 700 12px/1 var(--sans);
  letter-spacing: 0.12em;
  text-transform: uppercase;
}

.toggle-track {
  position: relative;
  width: 38px;
  height: 20px;
  border-radius: 999px;
  background: var(--line);
}

.toggle-thumb {
  position: absolute;
  top: 3px;
  left: 3px;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: var(--accent);
  transition: transform 0.2s ease;
}

.theme-toggle[aria-pressed='true'] .toggle-thumb {
  transform: translateX(18px);
}

.status::before {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  content: '';
}

.status.is-loading::before {
  background: #b08b54;
}

.status.is-ok::before {
  background: #27734c;
}

.status.is-warn::before {
  background: #c08222;
}

.status.is-error::before {
  background: #b5312e;
}

.status strong {
  color: var(--ink);
}

@media (max-width: 980px) {
  .topbar {
    align-items: start;
    flex-direction: column;
  }
}

@media (max-width: 620px) {
  .brand {
    align-items: flex-start;
    gap: 12px;
  }

  .brand-logo {
    width: 54px;
    height: 54px;
  }

  h1 {
    white-space: normal;
  }
}
</style>
