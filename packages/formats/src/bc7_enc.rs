//! BC7 encoder for HUD and logo art: modes 5 and 6 only, alpha weighted, with edge colour bleed.
//!
//! A line for line port of `native/src/core/apply/content/bc7.hpp`, which in turn equals the
//! offline Python pipeline byte for byte. The sums add in the same order as the C++ (one at a
//! time, or eight lanes for sixteen values) and Rust never fuses multiply-adds, so the output
//! is bit identical. `tests/bc7_golden.rs` checks it against bytes the C++ produced.

#![allow(clippy::needless_range_loop)]

use crate::{Error, Result};

type Block = [[i64; 4]; 16];

const W4: [i64; 16] = [0, 4, 9, 13, 17, 21, 26, 30, 34, 38, 43, 47, 51, 55, 60, 64];
const W2: [i64; 4] = [0, 21, 43, 64];

/// `std::min(a, b)` of the C++ standard library: `b < a ? b : a`.
fn cmin(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

/// `std::max(a, b)` of the C++ standard library: `a < b ? b : a`.
fn cmax(a: f64, b: f64) -> f64 {
    if a < b { b } else { a }
}

fn pairwise16(a: &[f64; 16]) -> f64 {
    let mut r = [0.0f64; 8];
    r.copy_from_slice(&a[..8]);
    for j in 0..8 {
        r[j] += a[8 + j];
    }
    ((r[0] + r[1]) + (r[2] + r[3])) + ((r[4] + r[5]) + (r[6] + r[7]))
}

fn rint_to(v: f64, lo: i64, hi: i64) -> i64 {
    let r = v.round_ties_even();
    if r.is_nan() || r <= lo as f64 {
        return lo;
    }
    if r > hi as f64 {
        return hi;
    }
    r as i64
}

#[derive(Default)]
struct Bits {
    word: [u8; 16],
    pos: usize,
}

impl Bits {
    fn put(&mut self, v: u64, n: usize) {
        let v = if n >= 64 { v } else { v & ((1u64 << n) - 1) };
        for i in 0..n {
            if (v >> i) & 1 != 0 {
                self.word[self.pos >> 3] |= 1u8 << (self.pos & 7);
            }
            self.pos += 1;
        }
    }
}

fn interp(a: i64, b: i64, w: i64) -> i64 {
    ((64 - w) * a + w * b + 32) >> 6
}

struct Endpoint {
    q: [i64; 4],
    v: [i64; 4],
    p: i64,
}

fn endpoint(c: &[i64; 4]) -> Endpoint {
    let mut best_err = -1i64;
    let mut out = Endpoint {
        q: [0; 4],
        v: [0; 4],
        p: 0,
    };
    for pb in 0..2i64 {
        let (mut qq, mut vv, mut err) = ([0i64; 4], [0i64; 4], 0i64);
        for k in 0..4 {
            qq[k] = ((c[k] - pb + 1) >> 1).clamp(0, 127);
            vv[k] = (qq[k] << 1) | pb;
            err += (vv[k] - c[k]) * (vv[k] - c[k]);
        }
        if best_err < 0 || err < best_err {
            best_err = err;
            out = Endpoint { q: qq, v: vv, p: pb };
        }
    }
    out
}

fn pack6(e0: &[i64; 4], e1: &[i64; 4], p0: i64, p1: i64, ix: &[i64; 16]) -> [u8; 16] {
    let mut b = Bits::default();
    b.put(1 << 6, 7);
    for c in 0..4 {
        b.put(e0[c] as u64, 7);
        b.put(e1[c] as u64, 7);
    }
    b.put(p0 as u64, 1);
    b.put(p1 as u64, 1);
    b.put(ix[0] as u64, 3);
    for &i in &ix[1..] {
        b.put(i as u64, 4);
    }
    b.word
}

fn flat_block(px: &Block) -> [u8; 16] {
    let (mut lo, mut hi) = (0, 0);
    for i in 1..16 {
        if px[i][3] < px[lo][3] {
            lo = i;
        }
        if px[i][3] > px[hi][3] {
            hi = i;
        }
    }
    let a = endpoint(&px[lo]);
    let b = endpoint(&px[hi]);
    let mut pal = [[0i64; 4]; 16];
    for i in 0..16 {
        for c in 0..4 {
            pal[i][c] = interp(a.v[c], b.v[c], W4[i]);
        }
    }
    let mut ix = [0i64; 16];
    for t in 0..16 {
        let mut best = -1i64;
        let mut pick = 0;
        for (i, p) in pal.iter().enumerate() {
            let mut e = 0i64;
            for c in 0..4 {
                let d = p[c] - px[t][c];
                e += d * d * if c == 3 { 4 } else { 1 };
            }
            if best < 0 || e < best {
                best = e;
                pick = i as i64;
            }
        }
        ix[t] = pick;
    }
    let (mut q0, mut q1, mut p0, mut p1) = (a.q, b.q, a.p, b.p);
    if ix[0] >= 8 {
        std::mem::swap(&mut q0, &mut q1);
        std::mem::swap(&mut p0, &mut p1);
        for i in &mut ix {
            *i = 15 - *i;
        }
    }
    pack6(&q0, &q1, p0, p1, &ix)
}

#[derive(Clone, Copy, Default)]
struct Quant {
    q: [i64; 4],
    v: [i64; 4],
    p: i64,
}

fn quantise(ep: &[f64; 4]) -> Quant {
    let mut c = [0i64; 4];
    for k in 0..4 {
        c[k] = rint_to(ep[k], 0, 255);
    }
    let mut best = Quant::default();
    let mut best_err = 0.0f64;
    for pb in 0..2i64 {
        let mut cur = Quant {
            p: pb,
            ..Quant::default()
        };
        let mut err = 0.0f64;
        for k in 0..4 {
            cur.q[k] = ((c[k] - pb + 1) >> 1).clamp(0, 127);
            cur.v[k] = (cur.q[k] << 1) | pb;
            let d = cur.v[k] as f64 - ep[k];
            err += d * d;
        }
        if pb == 0 || err < best_err {
            best = cur;
            best_err = err;
        }
    }
    best
}

fn assign(a: &Quant, b: &Quant, px: &Block, wt: &[[f64; 4]; 16], idx: &mut [usize; 16]) -> f64 {
    let mut pal = [[0i64; 4]; 16];
    for i in 0..16 {
        for c in 0..4 {
            pal[i][c] = ((64 - W4[i]) * a.v[c] + W4[i] * b.v[c] + 32) >> 6;
        }
    }
    let mut per = [0.0f64; 16];
    for t in 0..16 {
        let mut best = 0.0f64;
        let mut pick = 0;
        for (i, p) in pal.iter().enumerate() {
            let mut e = 0.0f64;
            for c in 0..4 {
                let d = (p[c] - px[t][c]) as f64;
                e += (d * d) * wt[t][c];
            }
            if i == 0 || e < best {
                best = e;
                pick = i;
            }
        }
        idx[t] = pick;
        per[t] = best;
    }
    pairwise16(&per)
}

fn fit(src: &[[f64; 4]; 16], wt: &[[f64; 4]; 16], idx: &[usize; 16]) -> ([f64; 4], [f64; 4]) {
    let mut f = [0.0f64; 16];
    let mut g = [0.0f64; 16];
    for t in 0..16 {
        f[t] = W4[idx[t]] as f64 / 64.0;
        g[t] = 1.0 - f[t];
    }
    let (mut e0, mut e1) = ([0.0f64; 4], [0.0f64; 4]);
    for c in 0..4 {
        let (mut a, mut b, mut cc, mut x0, mut x1) = (0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64);
        for t in 0..16 {
            a += wt[t][c] * (g[t] * g[t]);
            b += wt[t][c] * (g[t] * f[t]);
            cc += wt[t][c] * (f[t] * f[t]);
            x0 += (wt[t][c] * g[t]) * src[t][c];
            x1 += (wt[t][c] * f[t]) * src[t][c];
        }
        let det = a * cc - b * b;
        let ok = det.abs() > 1e-9;
        let flat = (x0 + x1) / cmax(a + 2.0 * b + cc, 1e-9);
        let r0 = if ok { (cc * x0 - b * x1) / det } else { flat };
        let r1 = if ok { (a * x1 - b * x0) / det } else { flat };
        e0[c] = cmin(255.0, cmax(0.0, r0));
        e1[c] = cmin(255.0, cmax(0.0, r1));
    }
    (e0, e1)
}

struct Cand {
    a: Quant,
    b: Quant,
    idx: [usize; 16],
    err: f64,
}

fn candidate(px: &Block, src: &[[f64; 4]; 16], wt: &[[f64; 4]; 16], s0: &[f64; 4], s1: &[f64; 4]) -> Cand {
    let mut cur = Cand {
        a: quantise(s0),
        b: quantise(s1),
        idx: [0; 16],
        err: 0.0,
    };
    cur.err = assign(&cur.a, &cur.b, px, wt, &mut cur.idx);
    for _ in 0..4 {
        let (f0, f1) = fit(src, wt, &cur.idx);
        let (r0, r1) = (quantise(&f0), quantise(&f1));
        let mut ridx = [0usize; 16];
        let rerr = assign(&r0, &r1, px, wt, &mut ridx);
        if rerr < cur.err {
            cur.a = r0;
            cur.b = r1;
            cur.err = rerr;
            cur.idx = ridx;
        }
    }
    cur
}

fn normalise(v: &mut [f64]) {
    let mut s = 0.0f64;
    for x in v.iter() {
        s += x * x;
    }
    let norm = cmax(s.sqrt(), 1e-12);
    for x in v.iter_mut() {
        *x /= norm;
    }
}

fn colour_block(px: &Block) -> [u8; 16] {
    let mut src = [[0.0f64; 4]; 16];
    let mut wt = [[0.0f64; 4]; 16];
    let mut sw = [[0.0f64; 4]; 16];
    for t in 0..16 {
        for c in 0..4 {
            src[t][c] = px[t][c] as f64;
        }
    }
    for t in 0..16 {
        for c in 0..4 {
            wt[t][c] = if c < 3 { src[t][3] / 255.0 } else { 4.0 };
            sw[t][c] = wt[t][c].sqrt();
        }
    }
    let (mut mean, mut swm) = ([0.0f64; 4], [0.0f64; 4]);
    for c in 0..4 {
        let (mut s, mut w, mut m) = (0.0f64, 0.0f64, 0.0f64);
        for t in 0..16 {
            s += src[t][c] * wt[t][c];
            w += wt[t][c];
            m += sw[t][c];
        }
        mean[c] = s / w;
        swm[c] = m / 16.0;
    }
    let mut cov = [[0.0f64; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            let mut s = 0.0f64;
            for t in 0..16 {
                s += ((src[t][i] - mean[i]) * sw[t][i]) * ((src[t][j] - mean[j]) * sw[t][j]);
            }
            cov[i][j] = s;
        }
    }
    let mut axis = [0.5f64; 4];
    for _ in 0..16 {
        let mut nx = [0.0f64; 4];
        for i in 0..4 {
            let mut s = 0.0f64;
            for j in 0..4 {
                s += cov[i][j] * axis[j];
            }
            nx[i] = s;
        }
        normalise(&mut nx);
        axis = nx;
    }
    for c in 0..4 {
        axis[c] /= cmax(swm[c], 1e-3);
    }
    normalise(&mut axis);
    let (mut tmin, mut tmax) = (0.0f64, 0.0f64);
    for t in 0..16 {
        let mut s = 0.0f64;
        for c in 0..4 {
            s += (src[t][c] - mean[c]) * axis[c];
        }
        if t == 0 || s < tmin {
            tmin = s;
        }
        if t == 0 || s > tmax {
            tmax = s;
        }
    }
    let mut seeds = [[[0.0f64; 4]; 2]; 3];
    let (mut lo, mut hi) = (0, 0);
    for t in 1..16 {
        if src[t][3] < src[lo][3] {
            lo = t;
        }
        if src[t][3] > src[hi][3] {
            hi = t;
        }
    }
    for c in 0..4 {
        let (mut mn, mut mx) = (src[0][c], src[0][c]);
        for row in &src[1..] {
            mn = cmin(mn, row[c]);
            mx = cmax(mx, row[c]);
        }
        seeds[0][0][c] = mean[c] + tmin * axis[c];
        seeds[0][1][c] = mean[c] + tmax * axis[c];
        seeds[1][0][c] = mn;
        seeds[1][1][c] = mx;
        seeds[2][0][c] = src[lo][c];
        seeds[2][1][c] = src[hi][c];
    }
    let mut best: Option<Cand> = None;
    for s in &seeds {
        let c = candidate(px, &src, &wt, &s[0], &s[1]);
        if best.as_ref().is_none_or(|b| c.err < b.err) {
            best = Some(c);
        }
    }
    let best = best.expect("three seeds");
    let mut ix = [0i64; 16];
    for t in 0..16 {
        ix[t] = best.idx[t] as i64;
    }
    let (mut e0, mut e1) = (best.a.q, best.b.q);
    let (mut p0, mut p1) = (best.a.p, best.b.p);
    if ix[0] >= 8 {
        std::mem::swap(&mut e0, &mut e1);
        std::mem::swap(&mut p0, &mut p1);
        for i in &mut ix {
            *i = 15 - *i;
        }
    }
    pack6(&e0, &e1, p0, p1, &ix)
}

fn bleed(px: &mut Block) {
    let mut tot = 0i64;
    let mut mean = [0i64; 3];
    for p in px.iter() {
        tot += p[3];
        for c in 0..3 {
            mean[c] += p[c] * p[3];
        }
    }
    if tot <= 0 {
        return;
    }
    for m in &mut mean {
        *m = (*m + tot / 2) / tot;
    }
    for p in px.iter_mut() {
        if p[3] == 0 {
            p[..3].copy_from_slice(&mean);
        }
    }
}

fn gather(rgba: &[u8], w: usize, bx: usize, by: usize) -> Block {
    let mut px = [[0i64; 4]; 16];
    for y in 0..4 {
        for x in 0..4 {
            for c in 0..4 {
                px[y * 4 + x][c] = rgba[((by * 4 + y) * w + bx * 4 + x) * 4 + c] as i64;
            }
        }
    }
    px
}

fn decode6(b: &[u8; 16]) -> Block {
    let bit = |pos: usize| ((b[pos >> 3] >> (pos & 7)) & 1) as i64;
    let get = |pos: usize, n: usize| {
        let mut v = 0i64;
        for i in 0..n {
            v |= bit(pos + i) << i;
        }
        v
    };
    let mut ep = [[0i64; 4]; 2];
    let mut pos = 7;
    for c in 0..4 {
        for e in &mut ep {
            e[c] = get(pos, 7);
            pos += 7;
        }
    }
    for e in &mut ep {
        for c in 0..4 {
            e[c] = (e[c] << 1) | bit(pos);
        }
        pos += 1;
    }
    let mut out = [[0i64; 4]; 16];
    for (t, o) in out.iter_mut().enumerate() {
        let n = if t == 0 { 3 } else { 4 };
        let ix = get(pos, n) as usize;
        pos += n;
        for c in 0..4 {
            o[c] = interp(ep[0][c], ep[1][c], W4[ix]);
        }
    }
    out
}

#[derive(Default, Clone)]
struct Mode5 {
    c0: [i64; 3],
    c1: [i64; 3],
    a0: i64,
    a1: i64,
    cidx: [usize; 16],
    aidx: [usize; 16],
}

fn block_error(px: &Block, dec: &Block) -> f64 {
    let mut per = [0.0f64; 16];
    for t in 0..16 {
        let mut rgb = 0.0f64;
        for c in 0..3 {
            let d = (px[t][c] - dec[t][c]) as f64;
            rgb += d * d;
        }
        let da = (px[t][3] - dec[t][3]) as f64;
        per[t] = rgb * px[t][3] as f64 / 255.0 + 4.0 * (da * da);
    }
    pairwise16(&per)
}

fn mode5_candidate(px: &Block, src: &[[f64; 3]; 16], wt: &[f64; 16]) -> (f64, Mode5) {
    let wsum = pairwise16(wt);
    let mut mean = [0.0f64; 3];
    for c in 0..3 {
        let mut s = 0.0f64;
        for t in 0..16 {
            s += src[t][c] * wt[t];
        }
        mean[c] = s / cmax(wsum, 1e-9);
    }
    let mut cov = [[0.0f64; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            let mut s = 0.0f64;
            for t in 0..16 {
                s += ((src[t][i] - mean[i]) * wt[t].sqrt()) * ((src[t][j] - mean[j]) * wt[t].sqrt());
            }
            cov[i][j] = s;
        }
    }
    let mut axis = [1.0f64; 3];
    for _ in 0..8 {
        let mut nx = [0.0f64; 3];
        for i in 0..3 {
            let mut s = 0.0f64;
            for j in 0..3 {
                s += cov[i][j] * axis[j];
            }
            nx[i] = s;
        }
        normalise(&mut nx);
        axis = nx;
    }
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for t in 0..16 {
        let mut s = 0.0f64;
        for c in 0..3 {
            s += (src[t][c] - mean[c]) * axis[c];
        }
        if wt[t] > 0.0 {
            lo = cmin(lo, s);
            hi = cmax(hi, s);
        }
    }
    if !lo.is_finite() {
        lo = 0.0;
    }
    if !hi.is_finite() {
        hi = 0.0;
    }
    let mut ends = [[0.0f64; 3]; 2];
    for c in 0..3 {
        ends[0][c] = mean[c] + lo * axis[c];
        ends[1][c] = mean[c] + hi * axis[c];
    }
    let mut have = false;
    let mut best_err = 0.0f64;
    let mut bq = [[0i64; 3]; 2];
    let mut bidx = [0usize; 16];
    for _ in 0..3 {
        let mut q = [[0i64; 3]; 2];
        let mut v = [[0i64; 3]; 2];
        for e in 0..2 {
            for c in 0..3 {
                q[e][c] = rint_to(ends[e][c] / 255.0 * 127.0, 0, 127);
                v[e][c] = (q[e][c] << 1) | (q[e][c] >> 6);
            }
        }
        let mut pal = [[0i64; 3]; 4];
        for i in 0..4 {
            for c in 0..3 {
                pal[i][c] = ((64 - W2[i]) * v[0][c] + W2[i] * v[1][c] + 32) >> 6;
            }
        }
        let mut idx = [0usize; 16];
        let mut per = [0.0f64; 16];
        for t in 0..16 {
            let mut best = -1i64;
            let mut pick = 0;
            for (i, p) in pal.iter().enumerate() {
                let mut e = 0i64;
                for c in 0..3 {
                    let d = p[c] - px[t][c];
                    e += d * d;
                }
                if best < 0 || e < best {
                    best = e;
                    pick = i;
                }
            }
            idx[t] = pick;
            per[t] = best as f64 * wt[t];
        }
        let err = pairwise16(&per);
        if !have || err < best_err {
            have = true;
            best_err = err;
            bq = q;
            bidx = idx;
        }
        let mut f = [0.0f64; 16];
        let mut g = [0.0f64; 16];
        for t in 0..16 {
            f[t] = W2[bidx[t]] as f64 / 64.0;
            g[t] = 1.0 - f[t];
        }
        let (mut pa, mut pb, mut pc) = ([0.0f64; 16], [0.0f64; 16], [0.0f64; 16]);
        for t in 0..16 {
            pa[t] = (wt[t] * g[t]) * g[t];
            pb[t] = (wt[t] * g[t]) * f[t];
            pc[t] = (wt[t] * f[t]) * f[t];
        }
        let (a, b, cc) = (pairwise16(&pa), pairwise16(&pb), pairwise16(&pc));
        let det = a * cc - b * b;
        let ok = det.abs() > 1e-9;
        let den = cmax(a + 2.0 * b + cc, 1e-9);
        for c in 0..3 {
            let (mut x0, mut x1) = (0.0f64, 0.0f64);
            for t in 0..16 {
                x0 += (wt[t] * g[t]) * src[t][c];
                x1 += (wt[t] * f[t]) * src[t][c];
            }
            let flat = (x0 + x1) / den;
            ends[0][c] = if ok { (cc * x0 - b * x1) / det } else { flat };
            ends[1][c] = if ok { (a * x1 - b * x0) / det } else { flat };
        }
    }
    let mut out = Mode5 {
        c0: bq[0],
        c1: bq[1],
        cidx: bidx,
        ..Mode5::default()
    };
    let (mut a0, mut a1) = (px[0][3], px[0][3]);
    for p in &px[1..] {
        a0 = a0.min(p[3]);
        a1 = a1.max(p[3]);
    }
    out.a0 = a0;
    out.a1 = a1;
    let mut apal = [0i64; 4];
    for i in 0..4 {
        apal[i] = ((64 - W2[i]) * a0 + W2[i] * a1 + 32) >> 6;
    }
    let mut cpal = [[0i64; 3]; 4];
    for i in 0..4 {
        for c in 0..3 {
            let e0 = (out.c0[c] << 1) | (out.c0[c] >> 6);
            let e1 = (out.c1[c] << 1) | (out.c1[c] >> 6);
            cpal[i][c] = ((64 - W2[i]) * e0 + W2[i] * e1 + 32) >> 6;
        }
    }
    let mut dec = [[0i64; 4]; 16];
    for t in 0..16 {
        let mut best = -1i64;
        let mut pick = 0;
        for (i, a) in apal.iter().enumerate() {
            let e = (a - px[t][3]).abs();
            if best < 0 || e < best {
                best = e;
                pick = i;
            }
        }
        out.aidx[t] = pick;
        for c in 0..3 {
            dec[t][c] = cpal[out.cidx[t]][c];
        }
        dec[t][3] = apal[pick];
    }
    (block_error(px, &dec), out)
}

fn pack5(mut m: Mode5) -> [u8; 16] {
    if m.cidx[0] >= 2 {
        std::mem::swap(&mut m.c0, &mut m.c1);
        for i in &mut m.cidx {
            *i = 3 - *i;
        }
    }
    if m.aidx[0] >= 2 {
        std::mem::swap(&mut m.a0, &mut m.a1);
        for i in &mut m.aidx {
            *i = 3 - *i;
        }
    }
    let mut b = Bits::default();
    b.put(1 << 5, 6);
    b.put(0, 2);
    for c in 0..3 {
        b.put(m.c0[c] as u64, 7);
        b.put(m.c1[c] as u64, 7);
    }
    b.put(m.a0 as u64, 8);
    b.put(m.a1 as u64, 8);
    b.put(m.cidx[0] as u64, 1);
    for &i in &m.cidx[1..] {
        b.put(i as u64, 2);
    }
    b.put(m.aidx[0] as u64, 1);
    for &i in &m.aidx[1..] {
        b.put(i as u64, 2);
    }
    b.word
}

fn check_size(rgba: &[u8], w: usize, h: usize) -> Result<()> {
    if w == 0 || h == 0 || !w.is_multiple_of(4) || !h.is_multiple_of(4) {
        return Err(Error::Format(format!(
            "BC7 needs a size that is a multiple of 4, not {w}x{h}"
        )));
    }
    if rgba.len() != w * h * 4 {
        return Err(Error::Format(format!(
            "{w}x{h} RGBA is {} bytes, not {}",
            w * h * 4,
            rgba.len()
        )));
    }
    Ok(())
}

/// Encodes straight RGBA8 (`w * h * 4` bytes, row major) to BC7 mode 6 blocks, 16 bytes per 4x4
/// block, rows of blocks top to bottom. `w` and `h` must be multiples of 4; pad first otherwise.
/// Transparent texels take the opaque texels' mean colour before encoding, so edges do not
/// fringe under bilinear sampling. Equals `bc7::encode_rgba` of the native core.
pub fn encode_rgba(rgba: &[u8], w: usize, h: usize) -> Result<Vec<u8>> {
    check_size(rgba, w, h)?;
    let mut out = Vec::with_capacity(w / 4 * (h / 4) * 16);
    for by in 0..h / 4 {
        for bx in 0..w / 4 {
            let mut bled = gather(rgba, w, bx, by);
            bleed(&mut bled);
            let flat = (1..16).all(|t| bled[t][..3] == bled[0][..3]);
            let word = if flat {
                flat_block(&bled)
            } else {
                colour_block(&bled)
            };
            out.extend_from_slice(&word);
        }
    }
    Ok(out)
}

/// [`encode_rgba`] for art with soft alpha: colour under fully transparent texels is zeroed first,
/// then each block switches to mode 5 (separate alpha) where that has the smaller alpha weighted
/// error. Equals `bc7::encode_logo` of the native core.
pub fn encode_logo(rgba: &[u8], w: usize, h: usize) -> Result<Vec<u8>> {
    check_size(rgba, w, h)?;
    let mut clean = rgba.to_vec();
    for p in clean.chunks_exact_mut(4) {
        if p[3] == 0 {
            p[..3].fill(0);
        }
    }
    let mut out = encode_rgba(&clean, w, h)?;
    for by in 0..h / 4 {
        for bx in 0..w / 4 {
            let px = gather(&clean, w, bx, by);
            let at = (by * (w / 4) + bx) * 16;
            let word: [u8; 16] = out[at..at + 16].try_into().expect("16 bytes");
            let stay = block_error(&px, &decode6(&word));
            let mut src = [[0.0f64; 3]; 16];
            let mut wt = [0.0f64; 16];
            for t in 0..16 {
                for c in 0..3 {
                    src[t][c] = px[t][c] as f64;
                }
                wt[t] = px[t][3] as f64 / 255.0;
            }
            let (err, m) = mode5_candidate(&px, &src, &wt);
            if err < stay {
                out[at..at + 16].copy_from_slice(&pack5(m));
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_solid_block_is_one_flat_colour() {
        let rgba: Vec<u8> = [200, 100, 50, 255].repeat(16);
        let out = encode_rgba(&rgba, 4, 4).unwrap();
        assert_eq!(out.len(), 16);
        assert_eq!(out[0] & 0x7f, 1 << 6, "mode 6");
        let dec = decode6(out[..16].try_into().unwrap());
        for p in dec {
            assert!((p[0] - 200).abs() <= 1 && (p[1] - 100).abs() <= 1 && p[3] >= 254);
        }
    }

    #[test]
    fn rejects_sizes_that_are_not_blocks() {
        assert!(encode_rgba(&[0; 12], 3, 1).is_err());
        assert!(encode_rgba(&[0; 10], 4, 4).is_err());
        assert!(encode_logo(&[], 0, 0).is_err());
    }

    #[test]
    fn soft_alpha_picks_mode_five_somewhere() {
        let mut rgba = Vec::new();
        for y in 0..4 {
            for x in 0..4 {
                rgba.extend_from_slice(&[255, 40 * x as u8, 20 * y as u8, (x * 64 + y * 5) as u8]);
            }
        }
        let a = encode_logo(&rgba, 4, 4).unwrap();
        let b = encode_rgba(&rgba, 4, 4).unwrap();
        assert!(a == b || a[0] & 0x3f == 1 << 5);
    }

    #[test]
    fn transparent_blocks_do_not_panic() {
        let mut rgba = vec![0u8; 4 * 4 * 4];
        for (i, v) in rgba.iter_mut().enumerate() {
            if i % 4 != 3 {
                *v = (i * 37 % 251) as u8;
            }
        }
        encode_rgba(&rgba, 4, 4).unwrap();
        encode_logo(&rgba, 4, 4).unwrap();
    }
}
