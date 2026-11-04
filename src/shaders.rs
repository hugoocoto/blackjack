//! The two shaders: a lamp that breathes slowly over the table (drawn with the
//! background, so it lights the felt but not the cards or the buttons), and
//! the old-screen look over the whole game (drawn when the game image is
//! scaled to the window).

use macroquad::prelude::*;

/// How much the screen bulges.
const CURVE: f32 = 0.02;

/// The point of the game image (0..1) that the curved screen shows at a point
/// of the window (0..1). The mouse goes through it too, so clicks land where
/// things appear. Same as `warp` in `SCREEN`.
pub fn warp(p: Vec2) -> Vec2 {
    let c = p * 2. - 1.;
    let c = c * vec2(1. + CURVE * c.y * c.y, 1. + CURVE * c.x * c.x);
    c * 0.5 + 0.5
}

/// How much the lamp lights a point of the game (in pixels), 0..1, before the
/// shader rounds it into steps. Same as `light` in `LAMP`.
pub fn lamp_light(p: Vec2, time: f32) -> f32 {
    let d = (p - vec2(240., 95.)) / vec2(250., 160.);
    let breathe = 0.5 + 0.5 * (time * std::f32::consts::TAU / 8.).sin();
    (-d.dot(d) * 1.8).exp() * (0.55 + 0.45 * breathe)
}

const VERTEX: &str = "#version 100
attribute vec3 position;
attribute vec2 texcoord;
varying lowp vec2 uv;
uniform mat4 Model;
uniform mat4 Projection;
void main() {
    gl_Position = Projection * Model * vec4(position, 1);
    uv = texcoord;
}
";

const SCREEN: &str = "#version 100
#ifdef GL_FRAGMENT_PRECISION_HIGH
precision highp float;
#else
precision mediump float;
#endif
varying vec2 uv;
uniform sampler2D Texture;
uniform vec2 src_size;
uniform vec2 dst_size;
uniform vec3 border;
uniform float time;

const float CURVE = 0.02;

vec2 warp(vec2 p) {
    vec2 c = p * 2.0 - 1.0;
    c *= vec2(1.0 + CURVE * c.y * c.y, 1.0 + CURVE * c.x * c.x);
    return c * 0.5 + 0.5;
}

// Crisp pixels (\"sharp bilinear\"): each game pixel is drawn solid, and only the
// screen pixel that straddles its edge is blended, so curved lines don't get
// jagged. `d` is the offset from the game pixel's centre; inside `r` it stays
// on the centre, and in the last screen pixel before the edge it slides
// towards the neighbour.
vec3 tex(vec2 p) {
    vec2 t = p * src_size;
    vec2 scale = dst_size / src_size;
    vec2 d = fract(t) - 0.5;
    vec2 r = 0.5 - 0.5 / scale;
    vec2 f = (d - clamp(d, -r, r)) * scale + 0.5;
    return texture2D(Texture, (floor(t) + f) / src_size).rgb;
}

void main() {
    vec2 p = warp(uv);

    // No colour fringing or glow: both smear the small pixel text.
    vec3 col = tex(p);

    // Scanlines, one per game pixel row; they fade out when the game is drawn
    // too small for them to look right.
    float s = dst_size.y / src_size.y;
    float row = fract(p.y * src_size.y) - 0.5;
    col *= 1.0 + smoothstep(1.5, 3.0, s) * (0.02 - 0.12 * row * row * 4.0);

    // A little darker in the far corners only, where nothing is printed.
    vec2 v = p * (1.0 - p);
    col *= mix(0.88, 1.0, pow(16.0 * v.x * v.y, 0.1));

    // Purple in the shadows, a little warmth in the highlights.
    float l = dot(col, vec3(0.299, 0.587, 0.114));
    col += vec3(0.03, 0.0, 0.05) * (1.0 - l) * (1.0 - l);
    col *= mix(vec3(1.0), vec3(1.04, 1.0, 0.95), l);

    // Very faint grain, changing a few times a second.
    vec2 cell = floor(uv * dst_size / 2.0);
    float n = fract(sin(dot(cell, vec2(12.9898, 78.233)) + floor(time * 8.0) * 7.31) * 43758.5453);
    col += (n - 0.5) * 0.012;

    // Rounded edges of the screen, smoothed over one screen pixel.
    vec2 e = min(p, 1.0 - p) * dst_size;
    float inside = clamp(min(e.x, e.y), 0.0, 1.0);
    gl_FragColor = vec4(mix(border, col, inside), 1.0);
}
";

const LAMP: &str = "#version 100
precision mediump float;
varying vec2 uv;
uniform sampler2D Texture;
uniform vec2 src_size;
uniform float time;
uniform float mood;

// Ordered dither: a fixed 4x4 pattern of thresholds.
float bayer2(vec2 a) {
    a = floor(a);
    return fract(a.x / 2.0 + a.y * a.y * 0.75);
}
float bayer4(vec2 a) {
    return bayer2(0.5 * a) * 0.25 + bayer2(a);
}

void main() {
    vec3 col = texture2D(Texture, uv).rgb;
    vec2 px = floor(uv * src_size);

    // A lamp over the middle of the table that breathes, once every 8 s.
    vec2 d = (px - vec2(240.0, 95.0)) / vec2(250.0, 160.0);
    float breathe = 0.5 + 0.5 * sin(time * 6.2831853 / 8.0);
    float light = exp(-dot(d, d) * 1.8) * (0.55 + 0.45 * breathe);

    // Light in a few steps, dithered, so it stays pixel art.
    light = floor(light * 8.0 + bayer4(px)) / 8.0;

    col *= 0.8 + 0.3 * light;
    col += vec3(0.04, 0.03, 0.0) * light;
    // Warmer and a touch brighter after a win, cooler after a loss.
    col += light * (max(mood, 0.0) * vec3(0.05, 0.025, -0.01) + max(-mood, 0.0) * vec3(-0.025, 0.0, 0.06));
    gl_FragColor = vec4(col, 1.0);
}
";

// Shadows that don't stack. The depth buffer marks which pixels are already
// in shadow: a shadow draws only where none is yet, and drawing a card (or a
// pile) clears the mark again, so a card can still catch one shadow from a
// card above it. Each shader puts its pixels at a fixed depth to do this.
const SHADE_VERTEX: &str = "#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;
varying lowp vec2 uv;
varying lowp vec4 color;
uniform mat4 Model;
uniform mat4 Projection;
void main() {
    gl_Position = Projection * Model * vec4(position, 1);
    gl_Position.z = -0.9 * gl_Position.w;
    uv = texcoord;
    color = color0 / 255.0;
}
";

const COVER_VERTEX: &str = "#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;
varying lowp vec2 uv;
varying lowp vec4 color;
uniform mat4 Model;
uniform mat4 Projection;
void main() {
    gl_Position = Projection * Model * vec4(position, 1);
    gl_Position.z = 0.9 * gl_Position.w;
    uv = texcoord;
    color = color0 / 255.0;
}
";

// Plain textured drawing; fully clear pixels (a card's round corners) are
// skipped so they neither shade nor clear anything.
const FLAT: &str = "#version 100
precision lowp float;
varying lowp vec2 uv;
varying lowp vec4 color;
uniform sampler2D Texture;
void main() {
    vec4 c = texture2D(Texture, uv) * color;
    if (c.a < 0.01) discard;
    gl_FragColor = c;
}
";

fn layer(vertex: &str, depth_test: Comparison) -> Option<Material> {
    let pipeline_params = PipelineParams {
        depth_test,
        depth_write: true,
        color_blend: Some(miniquad::BlendState::new(
            miniquad::Equation::Add,
            miniquad::BlendFactor::Value(miniquad::BlendValue::SourceAlpha),
            miniquad::BlendFactor::OneMinusValue(miniquad::BlendValue::SourceAlpha),
        )),
        ..Default::default()
    };
    load_material(ShaderSource::Glsl { vertex, fragment: FLAT }, MaterialParams { pipeline_params, ..Default::default() })
        .inspect_err(|e| eprintln!("hushjack: shader not available, drawing without it: {e:?}"))
        .ok()
}

/// Draw shadows with it: each pixel gets shaded once, however many overlap.
pub fn shade() -> Option<Material> {
    layer(SHADE_VERTEX, Comparison::Less)
}

/// Draw cards with it, so the next shadow can fall on them.
pub fn cover() -> Option<Material> {
    layer(COVER_VERTEX, Comparison::Always)
}

/// None if the graphics driver won't compile it; the game then draws without it.
fn load(fragment: &str, uniforms: &[(&str, UniformType)]) -> Option<Material> {
    let uniforms = uniforms.iter().map(|&(n, t)| UniformDesc::new(n, t)).collect();
    let shader = ShaderSource::Glsl { vertex: VERTEX, fragment };
    load_material(shader, MaterialParams { uniforms, ..Default::default() })
        .inspect_err(|e| eprintln!("hushjack: shader not available, drawing without it: {e:?}"))
        .ok()
}

/// The old-screen look; draw the game image with it.
pub fn screen() -> Option<Material> {
    load(
        SCREEN,
        &[
            ("src_size", UniformType::Float2),
            ("dst_size", UniformType::Float2),
            ("border", UniformType::Float3),
            ("time", UniformType::Float1),
        ],
    )
}

/// The breathing lamp; draw the background with it.
pub fn lamp() -> Option<Material> {
    load(LAMP, &[("src_size", UniformType::Float2), ("time", UniformType::Float1), ("mood", UniformType::Float1)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warp_keeps_the_middle_and_bends_the_corners_out() {
        assert_eq!(warp(vec2(0.5, 0.5)), vec2(0.5, 0.5));
        // The middle of each edge stays on the edge.
        assert!((warp(vec2(0., 0.5)) - vec2(0., 0.5)).length() < 1e-6);
        assert!((warp(vec2(1., 0.5)) - vec2(1., 0.5)).length() < 1e-6);
        // A window corner shows a point just outside the game image.
        let c = warp(vec2(1., 1.));
        assert!(c.x > 1. && c.y > 1.);
        // Symmetric, so the flipped render target doesn't matter.
        let (a, b) = (warp(vec2(0.2, 0.3)), warp(vec2(0.8, 0.7)));
        assert!((a + b - vec2(1., 1.)).length() < 1e-6);
    }
}
