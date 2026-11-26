struct Uniforms {
    view_projection: mat4x4<f32>,
    model: mat4x4<f32>,
    lighting: vec4<f32>,
    inverse_view_projection: mat4x4<f32>,
    atmosphere: vec4<f32>,
};
@group(0) @binding(0) var<uniform> uniforms: Uniforms;
fn sun_direction() -> vec3<f32> {
    let angle = (uniforms.atmosphere.x - 6.0) * 0.2617994;
    return normalize(vec3(cos(angle), sin(angle), 0.35));
}
fn daylight() -> f32 { return smoothstep(-0.18, 0.22, sun_direction().y); }
fn haze_color() -> vec3<f32> {
    let twilight = 1.0 - smoothstep(0.0, 0.4, abs(sun_direction().y));
    return mix(vec3(0.025, 0.035, 0.075), vec3(0.66, 0.79, 0.87), daylight())
        + twilight * vec3(0.27, 0.06, 0.01);
}
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) distance: f32,
};
@vertex fn vertex_main(@location(0) position: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) color: vec3<f32>) -> VertexOutput {
    var output: VertexOutput;
    output.position = uniforms.view_projection * uniforms.model * vec4(position, 1.0);
    output.color = color;
    output.normal = (uniforms.model * vec4(normal, 0.0)).xyz;
    output.distance = output.position.w;
    return output;
}
@fragment fn fragment_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let diffuse = abs(dot(normalize(input.normal), sun_direction()));
    let brightness = mix(1.0, 0.45 + 0.55 * diffuse, uniforms.lighting.x);
    let light = mix(vec3(0.28, 0.33, 0.46), vec3(1.0, 0.98, 0.92), daylight());
    let morning_mist = exp(-pow((uniforms.atmosphere.x - 6.0) / 2.0, 2.0));
    let fog = 1.0 - exp(-max(input.distance, 0.0) * mix(0.00035, 0.0012, morning_mist));
    return vec4(mix(input.color * brightness * light, haze_color(), fog), 1.0);
}
struct SkyOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};
@vertex fn sky_vertex(@builtin(vertex_index) index: u32) -> SkyOutput {
    var output: SkyOutput;
    let uv = vec2(f32((index << 1u) & 2u), f32(index & 2u));
    output.ndc = uv * 2.0 - 1.0;
    output.position = vec4(output.ndc, 1.0, 1.0);
    return output;
}
fn hash(p: vec2<f32>) -> f32 { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
fn noise(p: vec2<f32>) -> f32 {
    let cell = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(cell), hash(cell + vec2(1.0, 0.0)), u.x),
        mix(hash(cell + vec2(0.0, 1.0)), hash(cell + vec2(1.0)), u.x), u.y);
}
fn cloud_noise(p: vec2<f32>) -> f32 {
    return noise(p) * 0.55 + noise(p * 2.03) * 0.28 + noise(p * 4.07) * 0.12 + noise(p * 8.11) * 0.05;
}
@fragment fn sky_fragment(input: SkyOutput) -> @location(0) vec4<f32> {
    let near_h = uniforms.inverse_view_projection * vec4(input.ndc, 0.0, 1.0);
    let far_h = uniforms.inverse_view_projection * vec4(input.ndc, 1.0, 1.0);
    let ray = normalize(far_h.xyz / far_h.w - near_h.xyz / near_h.w);
    let day = daylight();
    let sun = sun_direction();
    let twilight = 1.0 - smoothstep(0.0, 0.4, abs(sun.y));
    let zenith = mix(vec3(0.006, 0.012, 0.035), vec3(0.17, 0.43, 0.76), day);
    var color = mix(haze_color(), zenith, pow(max(ray.y, 0.0), 0.55));
    let sun_dot = max(dot(ray, sun), 0.0);
    color += vec3(1.0, 0.38, 0.10) * pow(sun_dot, 12.0) * twilight * 0.5;
    color += vec3(1.0, 0.86, 0.57) * smoothstep(0.9994, 0.9998, sun_dot) * day;
    color += vec3(0.65, 0.73, 0.9) * smoothstep(0.9995, 0.99985, dot(ray, -sun)) * (1.0 - day);
    let star_cell = floor(vec2(atan2(ray.z, ray.x), asin(ray.y)) * 350.0);
    let star = pow(hash(star_cell), 180.0) * (1.0 - day) * smoothstep(0.05, 0.3, ray.y);
    color += vec3(star * 0.65);
    if ray.y > 0.005 {
        let wind = vec2(uniforms.atmosphere.y * 0.002, uniforms.atmosphere.y * 0.0007);
        let p = ray.xz / max(ray.y, 0.045) * 0.65 + wind;
        // Broader cloud cover around dawn, lighter scattered clouds at midday.
        let cover = 0.49 + 0.06 * sin((uniforms.atmosphere.x - 9.0) * 0.2617994);
        let density = smoothstep(cover, cover + 0.18, cloud_noise(p));
        let cloud = mix(vec3(0.07, 0.085, 0.14), vec3(0.94, 0.95, 0.97), day)
            + twilight * vec3(0.23, 0.08, 0.015);
        color = mix(color, cloud, density * smoothstep(0.03, 0.25, ray.y) * 0.93);
    }
    return vec4(color, 1.0);
}
