<script setup lang="ts">
import { ref } from 'vue'

const count = ref(0)
</script>

<template>
  <main class="playground">
    <section class="intro" aria-labelledby="title">
      <p class="eyebrow">Vue starter</p>
      <h1 id="title">Counter</h1>
      <p class="lede">A minimal Vue app running inside the ox-jsdoc playground.</p>
    </section>

    <section class="counter-card" aria-label="Counter controls">
      <p class="count">{{ count }}</p>
      <div class="actions">
        <button type="button" @click="count--">Decrement</button>
        <button type="button" @click="count++">Increment</button>
      </div>
    </section>
  </main>
</template>

<style scoped>
.playground {
  width: min(1180px, 100%);
  margin: 0 auto;
  padding: 56px 24px;
}

.intro {
  margin-bottom: 32px;
}

.eyebrow {
  margin: 0 0 12px;
  color: var(--accent);
  font: 700 13px/1 var(--mono);
  letter-spacing: 0.18em;
  text-transform: uppercase;
}

h1 {
  max-width: 760px;
  margin: 0;
  font-size: clamp(44px, 7vw, 88px);
  line-height: 0.92;
  letter-spacing: -0.06em;
}

.lede {
  max-width: 560px;
  margin: 20px 0 0;
  color: var(--muted);
  font-size: 18px;
}

.counter-card {
  display: grid;
  place-items: center;
  gap: 24px;
  width: min(520px, 100%);
  padding: 40px;
  border: 1px solid var(--line);
  border-radius: 20px;
  background: var(--panel);
  box-shadow: 0 24px 70px rgba(59, 45, 24, 0.13);
}

.count {
  margin: 0;
  color: var(--ink);
  font: 800 clamp(72px, 16vw, 132px)/0.9 var(--mono);
  letter-spacing: -0.08em;
}

.actions {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
  justify-content: center;
}

button {
  min-width: 132px;
  border: 1px solid var(--line);
  border-radius: 999px;
  padding: 12px 18px;
  background: var(--panel-strong);
  color: var(--ink);
  cursor: pointer;
  font: 700 14px/1 var(--sans);
}

button:hover {
  border-color: var(--accent);
  color: var(--accent);
}

button:focus-visible {
  outline: 3px solid var(--accent-soft);
  outline-offset: 3px;
}

@media (max-width: 840px) {
  .playground {
    padding: 36px 16px;
  }

  .counter-card {
    padding: 28px;
  }
}
</style>
