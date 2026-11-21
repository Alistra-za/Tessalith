"""Renders the per-effect True PBR balls on the Specular-Reference wiki page (docs/img/pbr-effects/*.png).

  py -3.13 -I tools/pbr-balls.py [--out docs/img/pbr-effects] [--size 128]

Needs numpy and Pillow. Writes one PNG per ball and overwrites the old ones; the same inputs give the same bytes.

Each ball is a unit sphere seen head-on, lit by one directional light, a two-colour ambient and a made-up sky for
reflections. The shading follows the pinned Community Shaders source in crates/render3d/shaders/cs/: the direct
lobes of PBR::GetDirectLightInput and the indirect weights of PBR::GetIndirectLobeWeights (Common/PBR.hlsli), the
material set-up and coat/fuzz/subsurface reads of Lighting.hlsl, GetParallaxCoords
(ExtendedMaterials/ExtendedMaterialsParallaxCore.hlsli) and the inner-layer refraction of Lighting.hlsl. What it
leaves out: shadows (parallax, contact and world shadows), cubemaps (a made-up sky stands in), wetness, vertex
colours, skylighting and CS's Glints2023 NDF (a random sparkle field per texel cell stands in). So the balls are
approximations for comparing effects with each other, not in-game captures. All textures are procedural.
"""
import argparse
import pathlib

import numpy as np
from PIL import Image

SS = 3  # supersampling per axis
BG = np.array([46, 48, 53], dtype=np.uint8)  # solid background, readable on light and dark wiki themes
LIGHT_DIR = np.array([-0.75, 0.55, 0.38])
LIGHT_DIR = LIGHT_DIR / np.linalg.norm(LIGHT_DIR)
LIGHT_COLOR = 4.0
SKY = np.array([0.22, 0.24, 0.28])
GROUND = np.array([0.06, 0.055, 0.05])
EPS_DOT = 1e-5
PI = np.pi


# ---------------------------------------------------------------- maths helpers

def dot(a, b):
    return np.sum(a * b, axis=-1)


def norm(v):
    return v / np.maximum(np.linalg.norm(v, axis=-1, keepdims=True), 1e-8)


def sat(x):
    return np.clip(x, 0.0, 1.0)


def lerp(a, b, t):
    return a + (b - a) * t


def srgb_to_linear(c):
    c = np.asarray(c, dtype=np.float64) / 255.0
    return np.where(c <= 0.04045, c / 12.92, ((c + 0.055) / 1.055) ** 2.4)


def linear_to_srgb8(c):
    c = sat(c)
    s = np.where(c <= 0.0031308, c * 12.92, 1.055 * np.power(c, 1 / 2.4) - 0.055)
    return np.round(s * 255.0).astype(np.uint8)


def tonemap(c):
    # Narkowicz's ACES fit: keeps highlights from clipping flat.
    return sat((c * (2.51 * c + 0.03)) / (c * (2.43 * c + 0.59) + 0.14))


def refract(i, n, eta):
    # HLSL refract(i, n, eta): i points at the surface.
    cosi = -dot(i, n)[..., None]
    k = 1.0 - eta**2 * (1.0 - cosi**2)
    return np.where(k < 0, 0.0, eta * i + (eta * cosi - np.sqrt(np.maximum(k, 0))) * n)


def hash2(ix, iy, salt):
    # Deterministic per-cell random numbers in [0, 1).
    h = np.sin(ix * 127.1 + iy * 311.7 + salt * 74.7) * 43758.5453
    return h - np.floor(h)


# ---------------------------------------------------------------- BRDF terms (Common/BRDF.hlsli)

def d_ggx(r, nh):
    a2 = (r * r) ** 2
    d = nh * nh * (a2 - 1.0) + 1.0
    return a2 / (PI * d * d)


def vis_smith_joint_approx(r, nv, nl):
    a = r * r
    return 0.5 / np.maximum(nl * (nv * (1 + a) + a) + nv * (nl * (1 + a) + a), 1e-6)


def f_schlick(f0, vh):
    fc = ((1 - vh) ** 5)[..., None]
    return fc + (1 - fc) * f0


def d_charlie(r, nh):
    inv = np.abs(r) ** -4
    sin2 = np.maximum(1.0 - nh * nh, 0.0)
    return (2.0 + inv) * np.power(sin2, inv * 0.5) / (2 * PI)


def vis_neubelt(nv, nl):
    return 1.0 / (4.0 * np.maximum(nl + nv - nl * nv, 1e-6))


def env_brdf(r, nv):
    # EnvBRDFApproxLazarov
    c0 = np.array([-1, -0.0275, -0.572, 0.022])
    c1 = np.array([1, 0.0425, 1.04, -0.04])
    rr = r[..., None] * c0 + c1
    a004 = np.minimum(rr[..., 0] ** 2, np.exp2(-9.28 * nv)) * rr[..., 0] + rr[..., 1]
    return -1.04 * a004 + rr[..., 2], 1.04 * a004 + rr[..., 3]


def multi_bounce_ao(base, ao):
    ao = ao[..., None]
    a = 2.0404 * base - 0.3324
    b = -4.7951 * base + 0.6417
    c = 2.7552 * base + 0.6903
    return np.maximum(ao, ((ao * a + b) * ao + c) * ao)


def specular_occlusion(nv, alpha, ao):
    return sat(np.power(np.abs(nv + ao), alpha) - 1.0 + ao)


def micro_shadow(ao, nl):
    # ApproximateDirectOcculusion; CS's default has micro-shadows on at strength 1.
    return sat((nl + 0.1) / np.sqrt(np.maximum(1.0000001 - ao, 1e-7)))


def microfacet(r, f0, nl, nv, nh, vh):
    f = f_schlick(f0, vh)
    return (d_ggx(r, nh) * vis_smith_joint_approx(r, nv, nl))[..., None] * f, f


# ---------------------------------------------------------------- environment stand-ins

def ambient(n):
    t = sat(0.5 + 0.5 * n[..., 1])[..., None]
    return lerp(GROUND, SKY, t)


def sky(r, rough):
    up = sat(r[..., 1])[..., None]
    horizon = np.array([0.75, 0.74, 0.72])
    zenith = np.array([0.22, 0.32, 0.55])
    col = lerp(GROUND * 1.5, lerp(horizon, zenith, np.sqrt(up)), sat(r[..., 1:2] / 0.35 + 0.5))
    box = norm(np.array([0.55, 0.45, 0.7]))
    w = 0.03 + 1.2 * rough**2
    ang = np.arccos(np.clip(dot(r, box), -1, 1))
    col = col + (3.0 * 0.03 / w * np.exp(-(ang**2) / w))[..., None]
    return lerp(col, ambient(r) * 1.6, sat(rough**1.5)[..., None])


# ---------------------------------------------------------------- procedural textures (texture space)

def tiles(p):
    """Brick height: 1 on a tile, 0 in the grout, with a bevel."""
    row = np.floor(p[..., 1])
    px = p[..., 0] + 0.5 * (np.mod(row, 2))
    fx = px - np.floor(px)
    fy = p[..., 1] - row
    d = np.minimum(np.minimum(fx, 1 - fx) * 0.5, np.minimum(fy, 1 - fy))
    t = sat((d - 0.02) / 0.06)
    return t * t * (3 - 2 * t)


def ripples(p):
    return 0.5 + 0.5 * np.sin(p[..., 0] * 2 * PI * 1.0 + np.sin(p[..., 1] * 2 * PI) * 1.5) * np.cos(p[..., 1] * 2 * PI * 1.3)


def height_normal(fn, p, strength):
    e = 1e-3
    dx = (fn(p + [e, 0]) - fn(p - [e, 0])) / (2 * e)
    dy = (fn(p + [0, e]) - fn(p - [0, e])) / (2 * e)
    return norm(np.stack([-dx * strength, dy * strength, np.ones_like(dx)], axis=-1))


TILE_SCALE = np.array([8.0, 4.0])  # texture repeats around and down the sphere


# ---------------------------------------------------------------- the renderer

def render(size, m):
    n_px = size * SS
    lin = (np.arange(n_px) + 0.5) / n_px * 2.3 - 1.15
    x, y = np.meshgrid(lin, -lin)
    r2 = x * x + y * y
    inside = r2 < 1.0
    z = np.sqrt(np.maximum(1.0 - r2, 0.0))
    geo_n = np.stack([x, y, z], axis=-1)
    v = np.broadcast_to(np.array([0.0, 0.0, 1.0]), geo_n.shape)

    lon = np.arctan2(x, z)
    lat = np.arcsin(np.clip(y, -1, 1))
    uv = np.stack([lon / (2 * PI) + 0.5, 0.5 - lat / PI], axis=-1) * TILE_SCALE
    t_vec = norm(np.stack([z, np.zeros_like(z), -x], axis=-1))
    b_vec = -np.cross(geo_n, t_vec)  # texture v runs down, so the bitangent points down

    def to_ts(w):
        return np.stack([dot(w, t_vec), -dot(w, b_vec), dot(w, geo_n)], axis=-1)

    def from_ts(n_ts):
        return norm(n_ts[..., 0:1] * t_vec - n_ts[..., 1:2] * b_vec + n_ts[..., 2:3] * geo_n)

    flags = m.get("flags", set())
    uv_orig = uv
    view = v
    eta = 1.0
    lit_dir = np.broadcast_to(LIGHT_DIR, geo_n.shape)
    base_light_dir = lit_dir

    # Parallax (Lighting.hlsl TRUE_PBR block + GetParallaxCoords).
    if "parallax" in m:
        height_fn = m["parallax"]
        scale = m.get("displacement_scale", 1.0)
        d_scale, d_off = 1.0, 0.0
        march_view = v
        if "interlayer" in flags:
            d_scale, d_off = 0.5, -0.25
            coat_f0 = m["coat_f0"]
            eta = lerp(1.0, (1 - np.sqrt(coat_f0)) / (1 + np.sqrt(coat_f0)), m["coat_strength"])
            march_view = -refract(-v, geo_n, eta)
            front = (dot(lit_dir, geo_n) > 0)[..., None]
            base_light_dir = np.where(front, -refract(-lit_dir, geo_n, eta), lit_dir)
        mv = norm(march_view)
        vts = np.stack([dot(mv, t_vec), dot(mv, b_vec), dot(mv, geo_n)], axis=-1)  # y along +v
        vxy = vts[..., :2] / (vts[..., 2:3] * 0.7 + 0.3)
        max_h = 0.1 * scale
        min_h = max_h * 0.5
        steps = 48
        prev_b, prev_h = np.ones(x.shape), np.ones(x.shape)
        hit_b = np.zeros(x.shape)
        found = np.zeros(x.shape, bool)
        for k in range(1, steps + 1):
            bound = 1.0 - k / steps
            off = vxy * (min_h - max_h * k / steps)
            h = (height_fn(uv + off) - 0.5) * d_scale + 0.5 + d_off
            new = (h >= bound) & ~found
            # Linear refinement between the last two samples, as CS does after its search.
            d1, d2 = bound - h, prev_b - prev_h
            den = np.where(np.abs(d2 - d1) < 1e-9, 1.0, d2 - d1)
            amt = (bound * d2 - prev_b * d1) / den
            hit_b = np.where(new, amt, hit_b)
            found |= new
            prev_b, prev_h = np.full(x.shape, bound), h
        hit_b = np.where(found, hit_b, 0.0)
        offset = (1.0 - hit_b) * -max_h + min_h
        uv = uv + vxy * offset[..., None]

    # Base textures, read at the (possibly displaced) uv.
    albedo = m.get("albedo_fn", lambda p: np.broadcast_to(srgb_to_linear(m.get("albedo", (150, 150, 150))), p.shape[:-1] + (3,)))(uv)
    rough = np.full(x.shape, m.get("roughness", 0.55))
    metal = np.full(x.shape, m.get("metallic", 0.0))
    ao = m.get("ao_fn", lambda p: np.ones(p.shape[:-1]))(uv)
    spec = np.full(x.shape, m.get("specular", 0.04))
    if "normal_fn" in m:
        n = from_ts(height_normal(m["normal_fn"], uv, m.get("normal_strength", 1.0)))
    else:
        n = geo_n

    # Material set-up (Lighting.hlsl, TRUE_PBR material block).
    rough = np.clip(rough, 0.04, 1.0)
    f0 = lerp(spec[..., None], albedo, metal[..., None])
    base = albedo * (1 - metal[..., None])

    coat_n = n
    if "coat" in flags:
        coat_rough = np.full(x.shape, m["coat_roughness"])
        coat_uv = uv_orig if "interlayer" in flags else uv
        if "coat_normal_fn" in m:
            coat_n = from_ts(height_normal(m["coat_normal_fn"], coat_uv, m.get("coat_normal_strength", 1.0)))
        coat_col = np.broadcast_to(srgb_to_linear(m.get("coat_color", (255, 255, 255))), base.shape)
        strength = m["coat_strength"]

    # Direct light: PBR::GetDirectLightInput.
    lc = LIGHT_COLOR
    vv = norm(view)
    ll = norm(base_light_dir)
    hh = norm(vv + ll)
    nl_raw = dot(n, ll)
    nl = np.clip(nl_raw, EPS_DOT, 1)
    nv = sat(np.abs(dot(n, vv)) + EPS_DOT)
    nh = sat(dot(n, hh))
    vh = sat(dot(vv, hh))
    vl = dot(vv, ll)
    diffuse = np.zeros(base.shape)
    specular = np.zeros(base.shape)
    transmission = np.zeros(base.shape)
    coat_diffuse = np.zeros(base.shape)

    if "glint" in flags:
        fr, f = glint_specular(m, rough, f0, nl, nv, nh, vh, to_ts(hh), uv_orig)
    else:
        fr, f = microfacet(rough, f0, nl, nv, nh, vh)
    kd = 1 - f
    diffuse += lc * nl[..., None] / PI * kd
    specular += fr * lc * nl[..., None]
    if "fuzz" in flags:
        fuzz_col = np.array(m["fuzz_color"])
        fz = (d_charlie(rough, nh) * vis_neubelt(nv, nl))[..., None] * f_schlick(fuzz_col, vh) * lc * nl[..., None]
        specular = lerp(specular, fz, m["fuzz_weight"])
    if "subsurface" in flags:
        thick = m["subsurface_opacity"]
        sc = np.array(m["subsurface_color"])
        if "foliage" in flags:
            sss = sat(-nl_raw) * (1 - thick)
        else:
            p = 12.234
            fwd = np.exp2(sat(-vl) * p - p)
            back = sat(nl * thick + (1 - thick)) * 0.5
            sss = lerp(back, 1.0, fwd) * (1 - thick)
        transmission += sc * sss[..., None] * lc / PI * kd
    elif "coat" in flags:
        cv, cl = v, lit_dir
        ch = norm(cv + cl)
        cnl = np.clip(dot(coat_n, cl), EPS_DOT, 1)
        cnv = sat(np.abs(dot(coat_n, cv)) + EPS_DOT)
        cnh = sat(dot(coat_n, ch))
        cvh = sat(dot(cv, ch))
        cfr, cf = microfacet(coat_rough, np.full(3, m["coat_f0"]), cnl, cnv, cnh, cvh)
        att = 1 - cf * strength
        diffuse *= att
        specular *= att
        coat_diffuse += lc * cnl[..., None] / PI
        specular += cfr * lc * cnl[..., None] * strength
    ms = micro_shadow(ao, nl_raw)[..., None]
    diffuse *= ms
    specular *= ms
    coat_diffuse *= ms

    # Indirect weights: PBR::GetIndirectLobeWeights.
    nv_i = sat(dot(n, vv))
    lobe_d = base.copy()
    if "subsurface" in flags:
        lobe_d += np.array(m["subsurface_color"]) * (1 - m["subsurface_opacity"]) / PI
    if "fuzz" in flags:
        lobe_d += np.array(m["fuzz_color"]) * m["fuzz_weight"]
    a, b = env_brdf(rough, nv_i)
    lobe_s = f0 * a[..., None] + b[..., None]
    lobe_d *= 1 - lobe_s
    if "coat" in flags:
        ca, cb = env_brdf(coat_rough, nv_i)
        cs = m["coat_f0"] * ca + cb
        att = 1 - cs[..., None] * strength
        lobe_d *= att
        lobe_s *= att
        if "coloured" in flags:
            lobe_d += coat_col * (1 - cs[..., None]) * strength
        lobe_s += cs[..., None] * strength
    lobe_d *= multi_bounce_ao(base, ao)
    lobe_s *= specular_occlusion(nv_i, rough * rough, ao)[..., None]

    # Composite (Lighting.hlsl, TRUE_PBR colour block).
    direct_d = diffuse * base
    if "coloured" in flags:
        direct_d = lerp(direct_d, coat_col * coat_diffuse, strength)
    color = direct_d + lobe_d * ambient(n) + transmission + specular
    refl = norm(2 * dot(n, vv)[..., None] * n - vv)
    color += lobe_s * sky(refl, rough)
    if "emissive_fn" in m:
        color += m["emissive_fn"](uv)

    img = np.where(inside[..., None], linear_to_srgb8(tonemap(color)), BG)
    pil = Image.fromarray(img.astype(np.uint8), "RGB")
    return pil.resize((size, size), Image.Resampling.BOX)


def glint_specular(m, rough, f0, nl, nv, nh, vh, h_ts, uv):
    """GGX with its D replaced by sparse sparkles: a stand-in for CS's Glints2023 NDF, not a port."""
    density = m["glint_cells"]
    cell = np.floor(uv * density)
    j1 = hash2(cell[..., 0], cell[..., 1], 1.0)
    j2 = hash2(cell[..., 0], cell[..., 1], 2.0)
    slope = (np.stack([j1, j2], axis=-1) - 0.5) * 2.0 * rough[..., None]
    hs = h_ts[..., :2] / np.maximum(h_ts[..., 2:3], 1e-3)
    sigma = m["glint_roughness"]
    sparkle = np.exp(-np.sum((hs - slope) ** 2, axis=-1) / (2 * sigma * sigma))
    d = d_ggx(rough, nh)
    d_glint = d * 0.35 + sparkle * d_ggx(np.full(rough.shape, 0.04), np.ones_like(nh)) * 0.02
    f = f_schlick(f0, vh)
    return (d_glint * vis_smith_joint_approx(rough, nv, nl))[..., None] * f, f


# ---------------------------------------------------------------- the balls

def tile_albedo(p):
    t = tiles(p)[..., None]
    return lerp(srgb_to_linear((70, 60, 52)), srgb_to_linear((176, 98, 70)), t)


def rune_glow(p):
    band = np.abs(np.sin(p[..., 1] * PI * 1.0))
    ring = sat(1 - np.abs(band - 0.15) / 0.06)
    tick = sat(1 - np.abs(np.sin(p[..., 0] * PI * 2.0)) / 0.25) * sat(1 - np.abs(band - 0.5) / 0.25)
    return np.maximum(ring, tick)[..., None] * np.array([2.2, 0.9, 0.25])


def sunk_tiles(p):
    """Tiles at mid-grey, grout black: flat under regular parallax, sunk under inner-layer parallax."""
    return 0.5 * tiles(p)


COAT = {"flags": {"coat"}, "coat_strength": 1.0, "coat_roughness": 0.15, "coat_f0": 0.04}

BALLS = {
    "baseline": {},
    "roughness": {"roughness": 0.15},
    "metallic": {"metallic": 1.0},
    "specular": {"specular": 0.17},
    "normal": {"normal_fn": tiles, "normal_strength": 0.6},
    "ao": {"ao_fn": lambda p: 0.15 + 0.85 * tiles(p)},
    "parallax": {"normal_fn": tiles, "normal_strength": 0.6, "parallax": tiles, "displacement_scale": 2.5},
    "emissive": {"emissive_fn": rune_glow},
    "subsurface": {"flags": {"subsurface"}, "subsurface_color": (0.95, 0.25, 0.12), "subsurface_opacity": 0.2},
    "foliage": {"flags": {"subsurface", "foliage"}, "subsurface_color": (0.35, 0.9, 0.15), "subsurface_opacity": 0.2},
    "coat": dict(COAT),
    "coat-colour": dict(COAT, flags={"coat", "coloured"}, coat_color=(60, 110, 230)),
    "coat-normal": dict(COAT, coat_normal_fn=ripples, coat_normal_strength=0.25),
    "coat-parallax-regular": dict(COAT, albedo_fn=tile_albedo, parallax=sunk_tiles, displacement_scale=3.0),
    "coat-parallax-inner": dict(COAT, flags={"coat", "interlayer"}, albedo_fn=tile_albedo, parallax=sunk_tiles, displacement_scale=3.0),
    "fuzz": {"flags": {"fuzz"}, "fuzz_color": (0.6, 0.6, 0.6), "fuzz_weight": 1.0},
    "glint": {"flags": {"glint"}, "roughness": 0.35, "glint_cells": 40.0, "glint_roughness": 0.02},
}


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--out", default="docs/img/pbr-effects")
    ap.add_argument("--size", type=int, default=128)
    args = ap.parse_args()
    out = pathlib.Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    for name, m in BALLS.items():
        path = out / f"{name}.png"
        render(args.size, m).save(path, optimize=True)
        print(f"{path} {path.stat().st_size} bytes")


if __name__ == "__main__":
    main()
