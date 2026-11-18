"""Renders the Swofty logo intro that replaces F1 25's EA SPORTS startup video
(videos/cm_f1_sting.bk2): 1920x1080, 30 fps, 6 s.

The logo is apps/launcher/assets/icon.svg rendered to a 2048 px PNG with a transparent
background (any SVG renderer; Edge or Chrome headless works:
  msedge --headless=new --default-background-color=00000000 --window-size=2048,2048 --screenshot=logo.png logo.html).
The glow is composited in float with fresh dither and light grain per frame, so it shows no
rings after the Bink encode. --voice mixes an optional voice line (any mono or stereo WAV) in under the chimes;
the shipped video has none.

  python make_intro.py --logo logo.png [--voice voice.wav] [--out DIR] [--ffmpeg ffmpeg]
  -> DIR/swofty_intro.mp4 (+ .wav and two stills)
  radvideo64 binkc DIR/swofty_intro.mp4 ../cm_f1_sting.bk2 /O /#
(RAD Video Tools; binkc writes Bink 2, the format the game's own video uses.)
"""
import argparse, os, subprocess, wave
import numpy as np
from PIL import Image, ImageDraw, ImageFont

W, H, FPS = 1920, 1080, 30
DUR = 6.0
NFR = int(DUR * FPS)
SR = 48000
OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "out")
FONT = r"C:\Windows\Fonts\segoeuib.ttf"
DEF_FF = "ffmpeg"
BG = np.array([14, 14, 20], np.float32)
PURPLE = np.array([0x71, 0x68, 0xAF], np.float32)   # icon.svg fill
LOGO_PX = 540
T_LOGO, T_WORD, T_OUT = 0.3, 1.2, 4.9


def smooth(t):
    t = min(max(t, 0.0), 1.0)
    return t * t * (3 - 2 * t)


def load_logo(path):
    im = Image.open(path).convert("RGBA")
    return im.crop(im.getbbox())


def wordmark():
    font = ImageFont.truetype(FONT, 92)
    tmp = Image.new("RGBA", (1400, 160), (0, 0, 0, 0))
    d = ImageDraw.Draw(tmp)
    x = 0
    for ch in "SWOFTY":
        d.text((x, 0), ch, font=font, fill=(235, 235, 245, 255))
        x += d.textbbox((0, 0), ch, font=font)[2] + 26
    return np.asarray(tmp.crop(tmp.getbbox()), np.float32) / 255


def over(fr, rgba, x0, y0, a):
    h, w = rgba.shape[:2]
    al = rgba[..., 3:4] * a
    fr[y0:y0 + h, x0:x0 + w] = fr[y0:y0 + h, x0:x0 + w] * (1 - al) + rgba[..., :3] * 255 * al


def render(pipe, logo_src):
    wm = wordmark()
    yy, xx = np.mgrid[0:H, 0:W].astype(np.float32)
    cy = H / 2 - 90
    r2 = ((xx - W / 2) / 1.0) ** 2 + ((yy - cy) / 1.15) ** 2
    glow = (np.exp(-r2 / (2 * 330.0 ** 2)) * 0.30)[..., None] * PURPLE[None, None]
    rng = np.random.default_rng(7)
    for i in range(NFR):
        t = i / FPS
        a_logo = smooth((t - T_LOGO) / 0.9)
        a_word = smooth((t - T_WORD) / 0.8)
        a_out = 1 - smooth((t - T_OUT) / 0.9)
        fr = np.broadcast_to(BG, (H, W, 3)).copy() + glow * (a_logo * a_out)
        if a_logo * a_out > 0:
            sc = 0.92 + 0.08 * smooth((t - T_LOGO) / 1.4) + 0.012 * max(0, t - 1.7) / 4.3
            sz = int(LOGO_PX * sc)
            lw = int(sz * logo_src.width / logo_src.height)
            lg = np.asarray(logo_src.resize((lw, sz), Image.LANCZOS), np.float32) / 255
            # light sweep over the mark (diagonal band, 1.7 s -> 2.6 s)
            p = smooth((t - 1.7) / 0.9)
            if 0 < p < 1:
                ly, lx = np.mgrid[0:sz, 0:lw].astype(np.float32)
                d = (lx / lw * 0.8 + ly / sz * 0.6) - (-0.3 + 1.9 * p)
                band = np.exp(-(d / 0.07) ** 2)[..., None] * 0.45
                lg = lg.copy()
                lg[..., :3] = lg[..., :3] + (1 - lg[..., :3]) * band
            over(fr, lg, int(W / 2 - lw / 2), int(cy - sz / 2), a_logo * a_out)
        if a_word * a_out > 0:
            over(fr, wm, int(W / 2 - wm.shape[1] / 2), int(H / 2 + 230 + 14 * (1 - a_word)), a_word * a_out)
        # triangular dither + light grain against banding (also survives Bink better)
        n = rng.random((H, W, 1), np.float32) + rng.random((H, W, 1), np.float32) - 1.0
        fr += n * 1.6
        pipe.write(np.clip(fr + 0.5, 0, 255).astype(np.uint8).tobytes())


def load_voice(path):
    with wave.open(path) as w:
        sr, ch, n = w.getframerate(), w.getnchannels(), w.getnframes()
        x = np.frombuffer(w.readframes(n), "<i2").astype(np.float64).reshape(-1, ch).mean(1) / 32768
    # trim silence
    idx = np.nonzero(np.abs(x) > 0.02)[0]
    x = x[max(0, idx[0] - int(0.01 * sr)): idx[-1] + int(0.05 * sr)]
    t_new = np.arange(int(len(x) * SR / sr)) / SR
    x = np.interp(t_new, np.arange(len(x)) / sr, x)
    # gentle low shelf lift (announcer body) + hall tail
    lp = np.convolve(x, np.ones(48) / 48, "same")
    x = x + 0.6 * lp
    rng = np.random.default_rng(3)
    L = int(1.4 * SR)
    ir = rng.standard_normal(L) * np.exp(-np.arange(L) / SR * 4.5)
    ir[: int(0.02 * SR)] = 0
    ir = np.convolve(ir, np.ones(24) / 24, "same")            # darker tail
    wet = np.convolve(x, ir)[: len(x) + L]
    wet = wet / (np.max(np.abs(wet)) + 1e-9)
    dry = np.concatenate([x / (np.max(np.abs(x)) + 1e-9), np.zeros(len(wet) - len(x))])
    return dry + 0.28 * wet


def audio(path, voice=None, t_voice=1.35):
    n = int(DUR * SR); t = np.arange(n) / SR
    out = np.zeros(n)
    sw = np.sin(2 * np.pi * 55 * t) * np.clip(t / 1.2, 0, 1) * np.exp(-np.maximum(0, t - 1.2) * 1.2)
    out += 0.35 * sw
    for t0, f in ((0.35, 659.25), (1.25, 987.77)):
        m = t >= t0; tt = t[m] - t0
        env = (1 - np.exp(-tt * 60)) * np.exp(-tt * 1.6)
        s = sum(a * np.sin(2 * np.pi * f * k * tt) for k, a in ((1, 1), (2, 0.35), (3, 0.12), (4.2, 0.05)))
        out[m] += 0.3 * env * s
    pad = sum(np.sin(2 * np.pi * f * t) for f in (164.81, 246.94, 329.63))
    out += 0.06 * pad * np.clip((t - 0.3) / 1.5, 0, 1) * np.clip((5.8 - t) / 1.2, 0, 1)
    out = out / np.max(np.abs(out)) * 0.55
    if voice:
        v = load_voice(voice)
        i0 = int(t_voice * SR); k = min(len(v), n - i0)
        out[i0:i0 + k] += 0.62 * v[:k]
    out *= np.clip((DUR - t) / 0.4, 0, 1)
    out = out / np.max(np.abs(out)) * 0.8
    pcm = (out * 32767).astype("<i2")
    with wave.open(path, "wb") as w:
        w.setnchannels(2); w.setsampwidth(2); w.setframerate(SR)
        w.writeframes(np.repeat(pcm[:, None], 2, 1).tobytes())


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--logo", required=True)
    ap.add_argument("--voice")
    ap.add_argument("--ffmpeg", default=DEF_FF)
    ap.add_argument("--shot", type=float, nargs="*", default=[2.1, 3.0])
    ap.add_argument("--out", default=OUT)
    a = ap.parse_args()
    os.makedirs(a.out, exist_ok=True)
    wav = os.path.join(a.out, "swofty_intro.wav"); audio(wav, a.voice)
    logo = load_logo(a.logo)
    mp4 = os.path.join(a.out, "swofty_intro.mp4")
    p = subprocess.Popen([a.ffmpeg, "-hide_banner", "-loglevel", "error", "-y",
                          "-f", "rawvideo", "-pix_fmt", "rgb24", "-s", f"{W}x{H}", "-r", str(FPS), "-i", "-",
                          "-i", wav, "-c:v", "libx264", "-crf", "12", "-tune", "grain", "-pix_fmt", "yuv420p",
                          "-c:a", "aac", "-b:a", "192k", "-shortest", mp4], stdin=subprocess.PIPE)
    render(p.stdin, logo)
    p.stdin.close()
    if p.wait():
        raise SystemExit("ffmpeg failed")
    for s in a.shot:
        subprocess.run([a.ffmpeg, "-hide_banner", "-loglevel", "error", "-y", "-ss", str(s), "-i", mp4,
                        "-frames:v", "1", os.path.join(a.out, f"shot_{s:.1f}s.png")], check=True)
    print("OK", mp4)


if __name__ == "__main__":
    main()
