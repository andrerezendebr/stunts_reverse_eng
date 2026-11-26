struct Uniforms {
    view_projection: mat4x4<f32>,
    model: mat4x4<f32>,
    lighting: vec4<f32>,
};
@group(0) @binding(0) var<uniform> uniforms: Uniforms;
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) normal: vec3<f32>,
};
@vertex fn vertex_main(@location(0) position: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) color: vec3<f32>) -> VertexOutput {
    var output: VertexOutput;
    output.position = uniforms.view_projection * uniforms.model * vec4(position, 1.0);
    output.color = color;
    output.normal = (uniforms.model * vec4(normal, 0.0)).xyz;
    return output;
}
@fragment fn fragment_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let diffuse = abs(dot(normalize(input.normal), normalize(vec3(0.4, 1.0, 0.3))));
    let brightness = mix(1.0, 0.45 + 0.55 * diffuse, uniforms.lighting.x);
    return vec4(input.color * brightness, 1.0);
}
