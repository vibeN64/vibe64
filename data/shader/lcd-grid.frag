#version 450

/*
    LCD grid: draws every source pixel as a slightly rounded cell with a thin dark gap
    around it, like a handheld screen. Needs the output to be several times bigger than
    the source: below about 3 output pixels per source pixel the gaps are too thin to draw.
*/

layout(push_constant) uniform Push
{
	vec4 SourceSize;
	vec4 OutputSize;
} params;

layout(location = 0) in vec2 vUV;
layout(location = 0) out vec4 FragColor;
layout(set = 0, binding = 0) uniform sampler2D Source;

// how dark the gap is, 0 = black, 1 = no gap
const float GAP_LEVEL = 0.25;
// brightness given back for the light lost in the gaps
const float GAIN = 1.2;

void main()
{
	vec2 pos = vUV * params.SourceSize.xy;
	vec2 cell = fract(pos);
	vec3 col = texture(Source, (floor(pos) + 0.5) * params.SourceSize.zw).rgb;

	// output pixels per source pixel: how wide a gap of one output pixel is in a cell
	vec2 scale = params.OutputSize.xy * params.SourceSize.zw;
	vec2 edge = max(vec2(0.12), 1.0 / scale);
	vec2 inside = smoothstep(vec2(0.0), edge, cell) * smoothstep(vec2(0.0), edge, 1.0 - cell);
	// cells smaller than 3 output pixels cannot show a gap
	float strength = clamp((min(scale.x, scale.y) - 2.0) / 2.0, 0.0, 1.0);

	float mask = inside.x * inside.y;
	col *= mix(1.0, mix(GAP_LEVEL, 1.0, mask) * GAIN, strength);
	FragColor = vec4(clamp(col, 0.0, 1.0), 1.0);
}
