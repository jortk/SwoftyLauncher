<script setup lang="ts">
import { reactive, ref } from "vue";

const API = import.meta.env.VITE_SWOFTY_API ?? "https://api.f1.swofty.net";

const form = reactive({
  name: "",
  organization: "",
  role: "owner",
  owner: "",
  email: "",
  phone: "",
  address: "",
  work: "",
  work_url: "",
  addons: "",
  material: "",
  good_faith: false,
  accurate: false,
  signature: "",
  website: "",
});

const sending = ref(false);
const error = ref("");
const sent = ref("");

async function send() {
  error.value = "";
  sending.value = true;
  try {
    const r = await fetch(`${API}/v1/dmca`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(form),
    });
    const body = await r.json().catch(() => ({}));
    if (!r.ok) throw new Error(body.error ?? `The server answered ${r.status}`);
    sent.value = body.id;
  } catch (e) {
    error.value = e instanceof TypeError ? "Couldn't reach the server. Try again in a minute." : String(e.message ?? e);
  } finally {
    sending.value = false;
  }
}
</script>

<template>
  <div v-if="sent" class="custom-block tip dmca-done" role="status">
    <p>
      <strong>Notice {{ sent }} sent.</strong> Keep this reference. Swofty Developments contacts you about it at
      {{ form.email }}.
    </p>
  </div>

  <form v-else class="dmca" @submit.prevent="send">
    <fieldset>
      <legend>You</legend>
      <label>
        <span>Full legal name</span>
        <input v-model="form.name" required maxlength="200" autocomplete="name" />
      </label>
      <label>
        <span>Company or organisation <em>(optional)</em></span>
        <input v-model="form.organization" maxlength="200" autocomplete="organization" />
      </label>
      <div class="dmca-roles" role="radiogroup" aria-label="Who you are">
        <label class="dmca-check">
          <input v-model="form.role" type="radio" value="owner" />
          <span>I am the copyright owner</span>
        </label>
        <label class="dmca-check">
          <input v-model="form.role" type="radio" value="agent" />
          <span>I am authorised to act for the copyright owner</span>
        </label>
      </div>
      <label v-if="form.role === 'agent'">
        <span>Copyright owner</span>
        <input v-model="form.owner" required maxlength="200" />
      </label>
      <label>
        <span>Email</span>
        <input v-model="form.email" type="email" required maxlength="254" autocomplete="email" />
      </label>
      <label>
        <span>Phone <em>(optional)</em></span>
        <input v-model="form.phone" type="tel" maxlength="50" autocomplete="tel" />
      </label>
      <label>
        <span>Mailing address</span>
        <textarea v-model="form.address" required maxlength="500" rows="3" autocomplete="street-address"></textarea>
      </label>
    </fieldset>

    <fieldset>
      <legend>Your work</legend>
      <label>
        <span>The copyrighted work</span>
        <small>What the work is and who made it. List each work if there are several.</small>
        <textarea v-model="form.work" required maxlength="2000" rows="4"></textarea>
      </label>
      <label>
        <span>Where the original can be seen <em>(optional)</em></span>
        <input v-model="form.work_url" type="url" maxlength="500" placeholder="https://" />
      </label>
    </fieldset>

    <fieldset>
      <legend>The addon</legend>
      <label>
        <span>Addon</span>
        <small>The name or id of each addon as the Addon Browser shows it, with the version if you know it.</small>
        <input v-model="form.addons" required maxlength="500" />
      </label>
      <label>
        <span>What in the addon infringes</span>
        <small>The files, assets or parts of the addon that copy your work.</small>
        <textarea v-model="form.material" required maxlength="3000" rows="5"></textarea>
      </label>
    </fieldset>

    <fieldset>
      <legend>Statements</legend>
      <label class="dmca-check">
        <input v-model="form.good_faith" type="checkbox" required />
        <span>
          I have a good faith belief that use of the material in the manner complained of is not authorised by the
          copyright owner, its agent, or the law.
        </span>
      </label>
      <label class="dmca-check">
        <input v-model="form.accurate" type="checkbox" required />
        <span>
          The information in this notice is accurate, and under penalty of perjury, I am the copyright owner or am
          authorised to act on behalf of the owner of an exclusive right that is allegedly infringed.
        </span>
      </label>
      <label>
        <span>Signature</span>
        <small>Type your full legal name. It is your electronic signature.</small>
        <input v-model="form.signature" required maxlength="200" />
      </label>
      <label class="dmca-trap" aria-hidden="true">
        <span>Website</span>
        <input v-model="form.website" tabindex="-1" autocomplete="off" />
      </label>
    </fieldset>

    <p v-if="error" class="dmca-error" role="alert">{{ error }}</p>
    <button type="submit" class="dmca-send" :disabled="sending">{{ sending ? "Sending..." : "Send notice" }}</button>
  </form>
</template>

<style>
.dmca fieldset {
  margin: 1.5rem 0 0;
  padding: 0;
  border: 0;
}

.dmca legend {
  margin-bottom: 0.25rem;
  color: var(--flash-white);
  font-size: 1.15rem;
  font-weight: 700;
}

.dmca label {
  display: block;
  margin-top: 0.9rem;
}

.dmca label > span {
  display: block;
  color: var(--flash-white);
  font-weight: 600;
}

.dmca label > span > em {
  color: var(--flash-less-light);
  font-style: normal;
  font-weight: 400;
}

.dmca label > small {
  display: block;
  color: var(--flash-less-light);
  font-size: 0.9rem;
  line-height: 1.5;
}

.dmca input:not([type="checkbox"], [type="radio"]),
.dmca textarea {
  display: block;
  width: 100%;
  margin-top: 0.3rem;
  padding: 0.5rem 0.75rem;
  border: 0.1rem solid transparent;
  border-radius: 0.25rem;
  background-color: var(--flash-gray-darkest);
  color: var(--flash-white);
  font: inherit;
}

.dmca textarea {
  resize: vertical;
}

.dmca input:focus-visible,
.dmca textarea:focus-visible,
.dmca-send:focus-visible {
  outline: 0.15rem solid var(--flash-highlight);
  outline-offset: 0.15rem;
}

.dmca .dmca-check {
  display: flex;
  align-items: flex-start;
  gap: 0.6rem;
  cursor: pointer;
}

.dmca .dmca-check > span {
  font-weight: 400;
  line-height: 1.5;
}

.dmca .dmca-check > input {
  flex: none;
  width: 1.1rem;
  height: 1.1rem;
  margin: 0.2rem 0 0;
  accent-color: var(--flash-highlight);
}

.dmca-roles {
  margin-top: 0.4rem;
}

.dmca-roles > .dmca-check {
  margin-top: 0.5rem;
}

.dmca .dmca-trap {
  position: absolute;
  left: -100vw;
  width: 1px;
  height: 1px;
  overflow: hidden;
}

.dmca-error {
  color: var(--flash-red);
}

.dmca-send {
  margin-top: 1.5rem;
  padding: 0.5rem 1.4rem;
  border-radius: 9999px;
  background-color: var(--flash-dark);
  box-shadow: 0 0.25rem 0.5rem var(--flash-shadow);
  color: var(--flash-light);
  font-weight: 600;
}

.dmca-send:hover:not(:disabled) {
  background-color: var(--flash-less-dark);
  color: var(--flash-white);
}

.dmca-send:disabled {
  opacity: 0.6;
  cursor: progress;
}
</style>
