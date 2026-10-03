#version 450

/*
    Scanlines: darkens the gaps between the lines of the picture, and lets the lines of
    brighter pixels grow wider, as on a CRT. Works in linear light so that the brightness
    stays right. Needs the output to be taller than the source; at or below that it
    leaves the picture alone, because there is no room to draw the gaps.
*/

layout(push_constant) uniform Push
{
	vec4 SourceSize;
	vec4 OutputSize;
} params;

layout(location = 0) in vec2 vUV;
layout(location = 0) out vec4 FragColor;
layout(set = 0, binding = 0) uniform sampler2D Source;

const float GAMMA = 2.2;
// how dark the gap between two lines is, 0 = black, 1 = no gap
const float GAP_LEVEL = 0.18;
const float GAIN = 1.3;

void main()
{
	float row = vUV.y * params.SourceSize.y;
	// sample the middle of the row, so that lines do not blend into each other
	vec2 uv = vec2(vUV.x, (floor(row) + 0.5) * params.SourceSize.w);
	vec3 col = pow(texture(Source, uv).rgb, vec3(GAMMA));

	// how many output pixels one source line covers
	float lines = params.OutputSize.y * params.SourceSize.w;
	// with fewer than 2.5 the gaps would turn into moire, so fade them out
	float strength = clamp((lines - 2.0) / 1.0, 0.0, 1.0);

	// 1 in the middle of the line, 0 at its edges
	float beam_pos = 0.5 - 0.5 * cos(6.28318530718 * fract(row));
	// bright pixels make a wider beam
	float lum = max(col.r, max(col.g, col.b));
	float beam = pow(beam_pos, mix(2.2, 0.9, lum));
	float shade = mix(GAP_LEVEL, 1.0, beam);

	col *= mix(1.0, shade * GAIN, strength);
	FragColor = vec4(pow(clamp(col, 0.0, 1.0), vec3(1.0 / GAMMA)), 1.0);
}
