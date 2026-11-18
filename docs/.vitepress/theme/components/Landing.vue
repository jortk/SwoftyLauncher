<script setup lang="ts">
import { computed } from "vue";
import { useData } from "vitepress";
import Icon from "./Icon.vue";
import DownloadButton from "./DownloadButton.vue";
import { useLinks } from "../links";

const { frontmatter: fm, theme } = useData();
const { to, asset, rel } = useLinks();

const repo = computed(() => theme.value.repo as string);
const hero = computed(() => fm.value.hero ?? {});
const shots = computed(() => fm.value.screenshots ?? {});
const year = new Date().getFullYear();

type Band = { color: string; at: number; amp: number; waves: number; phase: number };

function wave(band: Band, width: number, height: number) {
  const steps = 24;
  const pts: [number, number][] = [];
  for (let i = 0; i <= steps; i++) {
    const t = i / steps;
    const y =
      band.at +
      band.amp * Math.sin(t * Math.PI * 2 * band.waves + band.phase) +
      band.amp * 0.45 * Math.sin(t * Math.PI * 2 * (band.waves * 1.7 + 0.35) + band.phase * 1.9);
    pts.push([width * t, y]);
  }
  const f = (n: number) => n.toFixed(1);
  let d = `M0,${f(pts[0][1])}`;
  for (let i = 0; i < pts.length - 1; i++) {
    const p0 = pts[Math.max(0, i - 1)];
    const p1 = pts[i];
    const p2 = pts[i + 1];
    const p3 = pts[Math.min(pts.length - 1, i + 2)];
    d += ` C${f(p1[0] + (p2[0] - p0[0]) / 6)},${f(p1[1] + (p2[1] - p0[1]) / 6)} ${f(p2[0] - (p3[0] - p1[0]) / 6)},${f(p2[1] - (p3[1] - p1[1]) / 6)} ${f(p2[0])},${f(p2[1])}`;
  }
  return `${d} L${width},${height} L0,${height} Z`;
}

const heroBands: Band[] = [
  { color: "var(--lp-band-raspberry)", at: 90, amp: 22, waves: 1.1, phase: 0.6 },
  { color: "var(--lp-band-red)", at: 270, amp: 26, waves: 0.9, phase: 2.2 },
  { color: "var(--lp-band-coral)", at: 390, amp: 22, waves: 1.3, phase: 4.1 },
  { color: "var(--lp-band-peach)", at: 500, amp: 26, waves: 1.0, phase: 1.3 },
  { color: "var(--lp-band-cream)", at: 610, amp: 24, waves: 1.2, phase: 3.4 },
  { color: "var(--lp-band-dusk)", at: 735, amp: 30, waves: 0.8, phase: 5.2 },
  { color: "var(--lp-plum-top)", at: 790, amp: 22, waves: 1.15, phase: 0.2 },
];

const footBands: Band[] = [
  { color: "var(--lp-band-cream)", at: 40, amp: 22, waves: 0.9, phase: 2.6 },
  { color: "var(--lp-band-peach)", at: 175, amp: 24, waves: 1.2, phase: 0.9 },
  { color: "var(--lp-band-coral)", at: 300, amp: 22, waves: 1.0, phase: 4.4 },
  { color: "var(--lp-band-red)", at: 360, amp: 18, waves: 1.3, phase: 1.7 },
];

const heroPaths = heroBands.map((b) => ({ color: b.color, d: wave(b, 1440, 900) }));
const footPaths = footBands.map((b) => ({ color: b.color, d: wave(b, 1440, 420) }));
</script>

<template>
  <div class="lp">
    <a class="lp-skip" href="#lp-main">Skip to content</a>

    <nav class="lp-nav" aria-label="Site">
      <a class="lp-btn lp-btn--ink" :href="to('/')" aria-current="page"><Icon name="home" /><span>Home</span></a>
      <a class="lp-btn lp-btn--ink" :href="to(hero.docs)"><Icon name="book" /><span>Docs</span></a>
      <a class="lp-btn lp-btn--ink" href="https://publish.f1.swofty.net" aria-label="Publishing">
        <Icon name="upload" /><span class="lp-nav-label">Publishing</span>
      </a>
      <a class="lp-btn lp-btn--ink" :href="to('/dmca')" aria-label="DMCA" title="Report a copyright infringement">
        <Icon name="scale" /><span class="lp-nav-label">DMCA</span>
      </a>
    </nav>

    <header class="lp-hero">
      <svg class="lp-waves" viewBox="0 0 1440 900" preserveAspectRatio="none" aria-hidden="true">
        <rect width="1440" height="900" fill="var(--lp-band-wine)" />
        <path v-for="p in heroPaths" :key="p.d" :d="p.d" :fill="p.color" />
      </svg>

      <div class="lp-hero-inner">
        <div class="lp-brand">
          <svg class="lp-emblem" viewBox="0 0 64 64" aria-hidden="true">
            <rect x="5" y="5" width="54" height="54" rx="15" fill="none" stroke="currentColor" stroke-width="5" />
            <path
              d="M44 19H26.5a7.5 7.5 0 0 0 0 15h11a7.5 7.5 0 0 1 0 15H19"
              fill="none"
              stroke="currentColor"
              stroke-width="5.5"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
            <circle cx="46" cy="49" r="3.4" fill="currentColor" />
          </svg>
          <h1 class="lp-title">{{ hero.name }}</h1>
        </div>

        <p class="lp-tagline">
          <span v-html="hero.tagline" />
          <a class="lp-game" :href="hero.game?.link" rel="noopener"><Icon name="flag" /><span>{{ hero.game?.text }}</span></a>
        </p>

        <div class="lp-actions">
          <DownloadButton v-bind="hero.download" />
          <a class="lp-btn lp-btn--ink lp-btn--big" :href="to(hero.docs)"><Icon name="book" /><span>Docs</span></a>
        </div>
        <p class="lp-hint">Then follow the <a :href="to(hero.download.manual)">installation guide</a>.</p>
      </div>
    </header>

    <main id="lp-main" class="lp-main">
      <div class="lp-bokeh" aria-hidden="true" />
      <div class="lp-side lp-side--left" aria-hidden="true" />
      <div class="lp-side lp-side--right" aria-hidden="true" />

      <figure class="lp-shot lp-shot--hero">
        <img :src="asset(shots.hero.src)" :alt="shots.hero.alt" width="1920" height="1080" />
      </figure>

      <div class="lp-linkbox">
        <template v-for="(item, i) in fm.links" :key="item.text">
          <span v-if="i > 0" class="lp-dot" aria-hidden="true" />
          <a :href="to(item.link)" :rel="rel(item.link)">
            <span v-if="item.icon === 'github'" class="vpi-social-github lp-gh" />
            <Icon v-else :name="item.icon" />
            <span>{{ item.text }}</span>
          </a>
        </template>
      </div>

      <section class="lp-gallery" aria-label="Screenshots">
        <figure v-for="g in shots.gallery" :key="g.src" class="lp-shot" :class="`lp-shot--${g.shape}`">
          <img :src="asset(g.src)" :alt="g.alt" :width="g.shape === 'wide' ? 1920 : 1000" :height="g.shape === 'wide' ? 1080 : 888" loading="lazy" />
          <figcaption>{{ g.caption }}</figcaption>
        </figure>
      </section>

      <section class="lp-section lp-intro">
        <p v-html="fm.intro" />
        <ul class="lp-platforms">
          <li v-for="p in fm.platforms" :key="p.text" :title="p.text">
            <Icon :name="p.icon" /><span class="lp-sr">{{ p.text }}</span>
          </li>
        </ul>
      </section>

      <section class="lp-section" aria-labelledby="lp-features">
        <h2 id="lp-features" class="lp-heading" v-html="fm.featuresTitle" />
        <div class="lp-cards">
          <a v-for="f in fm.features" :key="f.title" class="lp-card" :href="to(f.link)">
            <span class="lp-card-icon"><Icon :name="f.icon" /></span>
            <h3>{{ f.title }}</h3>
            <p v-html="f.details" />
            <span class="lp-card-more">Read more <Icon name="arrow" /></span>
          </a>
        </div>
      </section>

      <section class="lp-section lp-split lp-split--code">
        <div class="lp-copy">
          <p v-html="fm.developers.text" />
          <div class="lp-row">
            <a
              v-for="a in fm.developers.actions"
              :key="a.text"
              class="lp-btn"
              :class="a.style === 'solid' ? 'lp-btn--rose' : 'lp-btn--rose-line'"
              :href="to(a.link)"
            >
              <Icon :name="a.icon" /><span>{{ a.text }}</span>
            </a>
          </div>
        </div>
        <div class="lp-code">
          <slot name="code" />
        </div>
      </section>

      <section class="lp-section lp-cta">
        <p v-html="fm.cta.text" />
        <div class="lp-row">
          <a v-for="a in fm.cta.actions" :key="a.text" class="lp-btn lp-btn--coral" :href="to(a.link)" :rel="rel(a.link)">
            <span v-if="a.icon === 'github'" class="vpi-social-github lp-gh" />
            <Icon v-else :name="a.icon" />
            <span>{{ a.text }}</span>
          </a>
        </div>
      </section>
    </main>

    <footer class="lp-footer">
      <svg class="lp-waves" viewBox="0 0 1440 420" preserveAspectRatio="none" aria-hidden="true">
        <path v-for="p in footPaths" :key="p.d" :d="p.d" :fill="p.color" />
      </svg>
      <div class="lp-footer-inner">
        <div class="lp-linkrow">
          <a :href="to('/guide/')"><Icon name="book" /><span>Documentation</span></a>
          <span class="lp-dot" aria-hidden="true" />
          <a :href="repo" rel="noopener"><span class="vpi-social-github lp-gh" /><span>Source Code</span></a>
          <span class="lp-dot" aria-hidden="true" />
          <a :href="`${repo}/releases`" rel="noopener"><Icon name="tag" /><span>Releases</span></a>
          <span class="lp-dot" aria-hidden="true" />
          <a :href="`${repo}/blob/main/LICENSE`" rel="noopener"><Icon name="scale" /><span>Licence</span></a>
        </div>
        <p>Released under the PolyForm Noncommercial 1.0.0 licence. The SDK and the addon template are MIT.</p>
        <p>Not affiliated with Codemasters, Electronic Arts or Formula 1.</p>
        <p class="lp-credit">
          Styled after <a href="https://geode-sdk.org/" rel="noopener">Geode</a>. The docs reuse CSS from the
          <a href="https://github.com/geode-sdk/docs" rel="noopener">Geode docs</a> under the MIT licence; see
          <a :href="`${repo}/blob/main/THIRD_PARTY.md`" rel="noopener">THIRD_PARTY.md</a>.
        </p>
        <p>
          <a :href="to('/terms')">Terms of Service</a> and <a :href="to('/privacy')">Privacy Policy</a>
        </p>
        <p class="lp-copyright">&copy; {{ year }} Swofty</p>
      </div>
    </footer>
  </div>
</template>
