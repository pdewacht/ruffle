/// Shader used for drawing a pending framebuffer onto a parent framebuffer

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

struct BlendOptions {
    mode: i32,
    _padding1: f32,
    _padding2: f32,
    _padding3: f32,
}

@group(2) @binding(0) var parent_texture: texture_2d<f32>;
@group(2) @binding(1) var current_texture: texture_2d<f32>;
@group(2) @binding(2) var texture_sampler: sampler;
@group(2) @binding(3) var<uniform> blend: BlendOptions;

@vertex
fn main_vertex(in: VertexInput) -> VertexOutput {
    let pos = globals.view_matrix * transforms.world_matrix * vec4<f32>(in.position.x, in.position.y, 1.0, 1.0);
    let uv = vec2<f32>((pos.x + 1.0) / 2.0, -((pos.y - 1.0) / 2.0));
    return VertexOutput(pos, uv);
}

@fragment
fn main_fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    var parent: vec4<f32> = textureSample(parent_texture, texture_sampler, in.uv);
    var current: vec4<f32> = textureSample(current_texture, texture_sampler, in.uv);

    switch (blend.mode) {
        case 3: { // multiply
            if (current.a > 0.0) {
                current = vec4<f32>(mix(parent.rgb, current.rgb / current.a, current.a), 1.0);
                return vec4<f32>(current.rgb * parent.rgb, current.a);
            } else {
                discard;
            }
        }
        case 4: { // screen
            if (current.a > 0.0) {
                return vec4<f32>((parent.rgb + current.rgb) - (parent.rgb * current.rgb), current.a);
            } else {
                discard;
            }
        }
        case 5: { // lighten
            if (current.a > 0.0) {
                return vec4<f32>(max(current.rgb, parent.rgb), current.a);
            } else {
                discard;
            }
        }
        case 6: { // darken
            if (current.a > 0.0) {
                return vec4<f32>(min(current.rgb, parent.rgb), current.a);
            } else {
                discard;
            }
        }
        case 7: { // difference
            if (current.a > 0.0) {
                return vec4<f32>(abs(parent.rgb - current.rgb), current.a);
            } else {
                discard;
            }
        }
        case 8: { // add
            if (current.a > 0.0) {
                return vec4<f32>(current.rgb + parent.rgb, current.a);
            } else {
                discard;
            }
        }
        case 9: { // subtract
            if (current.a > 0.0) {
                return vec4<f32>(parent.rgb - current.rgb, current.a);
            } else {
                discard;
            }
        }
        case 10: { // invert
            if (current.a > 0.0) {
                return vec4<f32>(1.0 - parent.rgb, current.a);
            } else {
                discard;
            }
        }
        case 11: { // alpha
            let alpha = current.a * parent.a;
            return vec4<f32>(parent.rgb / parent.a * alpha, alpha);
        }
        case 12: { // erase
            let alpha = (1.0 - current.a) * parent.a;
            return vec4<f32>(parent.rgb / parent.a * alpha, alpha);
        }
        case 13: { // overlay
            if (current.a > 0.0) {
                var r: f32;
                var g: f32;
                var b: f32;
                current = vec4<f32>(current.rgb / current.a, current.a);
                parent = vec4<f32>(parent.rgb / parent.a, parent.a);
                current = vec4<f32>(mix(parent.rgb, current.rgb, current.a), 1.0);
                if (parent.r < 0.5) {
                    r = (2.0 * current.r * parent.r);
                } else {
                    r = (1.0 - 2.0 * (1.0 - parent.r) * (1.0 - current.r));
                }
                if (parent.g < 0.5) {
                    g = (2.0 * current.g * parent.g);
                } else {
                    g = (1.0 - 2.0 * (1.0 - parent.g) * (1.0 - current.g));
                }
                if (parent.b < 0.5) {
                    b = (2.0 * current.b * parent.b);
                } else {
                    b = (1.0 - 2.0 * (1.0 - parent.b) * (1.0 - current.b));
                }
                return vec4<f32>(r * current.a, g * current.a, b * current.a, current.a);
            } else {
                discard;
            }
        }
        case 14: { // hardlight
            if (current.a > 0.0) {
                var r: f32;
                var g: f32;
                var b: f32;
                if (current.r <= 0.5) {
                    r = current.r * parent.r;
                } else {
                    r = (parent.r + current.r) - (parent.r * current.r);
                }
                if (current.g <= 0.5) {
                    g = current.g * parent.g;
                } else {
                    g = (parent.g + current.g) - (parent.g * current.g);
                }
                if (current.b <= 0.5) {
                    b = current.b * parent.b;
                } else {
                    b = (parent.b + current.b) - (parent.b * current.b);
                }
                return vec4<f32>(r, g, b, current.a);
            } else {
                discard;
            }
        }
        default: {
            if (current.a > 0.0) {
                current = vec4<f32>(mix(parent.rgb, current.rgb / current.a, current.a), 1.0);
                return current;
            } else {
                discard;
            }
        }
    }
    return parent;
}
