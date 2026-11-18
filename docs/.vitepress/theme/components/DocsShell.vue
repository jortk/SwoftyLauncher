<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { useData, useRoute, withBase } from "vitepress";
import { VPNavBarSearch, VPSocialLink } from "vitepress/theme-without-fonts";
import Icon from "./Icon.vue";
import NavItem from "./NavItem.vue";
import type { DocsThemeConfig, NavItem as Item } from "../types";
import { useLinks } from "../links";

const { theme, page, frontmatter } = useData<DocsThemeConfig>();
const route = useRoute();
const { to, samePage } = useLinks();

const tabs = computed(() => theme.value.tabs);

function contains(items: Item[]): boolean {
  return items.some((i) => samePage(i.link, route.path) || contains(i.items ?? []));
}

const pageTab = computed(() => tabs.value.find((t) => t.groups.some((g) => contains(g.items)))?.id ?? tabs.value[0].id);
const tab = ref(pageTab.value);
watch(pageTab, (v) => (tab.value = v));
const currentTab = computed(() => tabs.value.find((t) => t.id === tab.value) ?? tabs.value[0]);

const menuOpen = ref(false);
const hash = ref("");
const query = ref("");

function syncHash() {
  hash.value = decodeURIComponent(window.location.hash);
}

watch(
  () => route.path,
  async () => {
    menuOpen.value = false;
    await nextTick();
    syncHash();
  },
);

type Result = { item: Item; scope: string; mono: boolean };

const results = computed<Result[]>(() => {
  const q = query.value.trim().toLowerCase();
  if (!q) return [];
  const out: Result[] = [];
  const walk = (items: Item[], scope: string, mono: boolean) => {
    for (const item of items) {
      if (item.text.toLowerCase().includes(q)) out.push({ item, scope, mono });
      if (item.items) walk(item.items, item.text, mono);
    }
  };
  for (const t of tabs.value) for (const g of t.groups) walk(g.items, g.text, !!t.mono);
  return out.slice(0, 60);
});

function parts(text: string) {
  const q = query.value.trim();
  const i = text.toLowerCase().indexOf(q.toLowerCase());
  if (!q || i < 0) return [{ text, match: false }];
  return [
    { text: text.slice(0, i), match: false },
    { text: text.slice(i, i + q.length), match: true },
    { text: text.slice(i + q.length), match: false },
  ];
}

const themes = [
  { id: "dark", icon: "moon", label: "Dark theme" },
  { id: "peach", icon: "feather", label: "Peach theme" },
  { id: "ocean", icon: "umbrella", label: "Ocean theme" },
];
const current = ref("peach");

function pick(id: string) {
  const root = document.documentElement;
  for (const t of themes) root.classList.remove(`flash-theme-${t.id}`);
  root.classList.add(`flash-theme-${id}`);
  current.value = id;
  try {
    localStorage.setItem("f1mm-docs-theme", id);
  } catch {}
}

const sidebarClosed = ref(false);

function toggleSidebar() {
  sidebarClosed.value = !sidebarClosed.value;
  document.documentElement.classList.toggle("flash-sidebar-closed", sidebarClosed.value);
  try {
    localStorage.setItem("f1mm-docs-sidebar", sidebarClosed.value ? "closed" : "open");
  } catch {}
}

onMounted(() => {
  sidebarClosed.value = document.documentElement.classList.contains("flash-sidebar-closed");
  const found = themes.find((t) => document.documentElement.classList.contains(`flash-theme-${t.id}`));
  if (found) current.value = found.id;
  syncHash();
  window.addEventListener("hashchange", syncHash);
});

onUnmounted(() => window.removeEventListener("hashchange", syncHash));
</script>

<template>
  <div class="flash-docs" :class="{ 'menu-open': menuOpen, wide: frontmatter.wide }">
    <a class="skip-link" href="#flash-main">Skip to content</a>

    <nav id="flash-nav" :class="{ collapsed: !menuOpen }" aria-label="Documentation">
      <header>
        <a :href="to('/')">
          <img :src="withBase('/logo.svg')" alt="" width="36" height="36" />
          <span>Swofty Launcher for F1 25</span>
        </a>
        <VPSocialLink class="button" icon="github" :link="theme.repo" aria-label="Source code on GitHub" />
      </header>

      <div class="mode" role="tablist">
        <button
          v-for="t in tabs"
          :key="t.id"
          role="tab"
          :aria-selected="t.id === tab"
          :class="{ selected: t.id === tab }"
          @click="tab = t.id"
        >
          <Icon :name="t.icon" /><span>{{ t.text }}</span><span />
        </button>
      </div>

      <div v-if="query.trim()" class="content" :class="{ monospace: currentTab.mono }">
        <a v-for="r in results" :key="r.item.link" :href="to(r.item.link)" :class="{ mono: r.mono }">
          <Icon :name="r.item.icon ?? 'bookmark'" />
          <span>
            <span class="scope">{{ r.scope }} / </span>
            <template v-for="(p, i) in parts(r.item.text)" :key="i">
              <span v-if="p.match" class="matched">{{ p.text }}</span>
              <template v-else>{{ p.text }}</template>
            </template>
          </span>
        </a>
        <p v-if="!results.length" class="nothing-found">Nothing found</p>
      </div>
      <div v-else class="content" :class="{ monospace: currentTab.mono }">
        <details v-for="g in currentTab.groups" :key="g.text" :class="{ root: currentTab.mono }" open>
          <summary><Icon name="chevron" class="feather-chevron-right" /><span>{{ g.text }}</span></summary>
          <div>
            <NavItem v-for="item in g.items" :key="item.link" :item="item" :hash="hash" />
          </div>
        </details>
      </div>

      <div class="search">
        <div class="input">
          <input v-model="query" type="search" placeholder="Search items..." aria-label="Search items" />
          <button v-if="query" type="button" aria-label="Clear search" @click="query = ''"><Icon name="x" /></button>
          <button v-else type="button" tabindex="-1" aria-hidden="true"><Icon name="search" /></button>
        </div>
      </div>
    </nav>

    <main id="flash-main">
      <div v-if="page.isNotFound" class="text vp-doc">
        <h1>Page not found</h1>
        <p>There is no page at this address. It may have moved when the docs were reorganised.</p>
        <p><a :href="to('/guide/')">Go to the documentation home</a></p>
      </div>
      <div v-else class="text vp-doc">
        <Content />
      </div>
    </main>

    <div class="overlay menu">
      <button type="button" :aria-expanded="menuOpen" aria-label="Toggle navigation" @click="menuOpen = !menuOpen">
        <Icon :name="menuOpen ? 'x' : 'menu'" />
      </button>
    </div>

    <div class="overlay sidebar">
      <button
        type="button"
        aria-controls="flash-nav"
        :aria-expanded="!sidebarClosed"
        :aria-label="sidebarClosed ? 'Show sidebar' : 'Hide sidebar'"
        :title="sidebarClosed ? 'Show sidebar' : 'Hide sidebar'"
        @click="toggleSidebar"
      >
        <Icon name="panel-left-close" class="when-open" />
        <Icon name="panel-left-open" class="when-closed" />
      </button>
    </div>

    <div class="overlay actions">
      <a
        class="publish"
        href="https://publish.f1.swofty.net"
        aria-label="Publishing"
        title="Publish addons"
      >
        <Icon name="upload" /><span>Publishing</span>
      </a>
      <a
        class="publish"
        :class="{ selected: samePage('/dmca', route.path) }"
        :href="to('/dmca')"
        aria-label="DMCA"
        title="Report a copyright infringement"
      >
        <Icon name="scale" /><span>DMCA</span>
      </a>
      <VPNavBarSearch />
    </div>

    <div class="overlay theme" role="group" aria-label="Theme">
      <button
        v-for="t in themes"
        :key="t.id"
        type="button"
        :class="{ selected: current === t.id }"
        :aria-pressed="current === t.id"
        :aria-label="t.label"
        :title="t.label"
        @click="pick(t.id)"
      >
        <Icon :name="t.icon" />
      </button>
    </div>
  </div>
</template>
