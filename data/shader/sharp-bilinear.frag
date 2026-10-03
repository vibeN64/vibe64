#version 450

/*
    Sharp bilinear: scales every source pixel up by a whole number first, then lets
    the graphics hardware blend only the thin seam between pixels. The picture stays
    crisp, like nearest-neighbour, but without the uneven pixel widths that appear at
    scales that are not whole numbers. Meant for a linear-filtered sampler.
*/

layout(push_constant) uniform Push
{
	vec4 SourceSize;
	vec4 OutputSize;
} params;

layout(location = 0) in vec2 vUV;
layout(location = 0) out vec4 FragColor;
layout(set = 0, binding = 0) uniform sampler2D Source;

void main()
{
	vec2 texel = vUV * params.SourceSize.xy;
	// whole-number part of the scale, at least 1
	vec2 scale = max(floor(params.OutputSize.xy * params.SourceSize.zw + 0.01), vec2(1.0));

	vec2 texel_floored = floor(texel);
	vec2 s = fract(texel);
	// inside this distance from the middle of a pixel the colour is flat
	vec2 region_range = 0.5 - 0.5 / scale;
	vec2 center_dist = s - 0.5;
	vec2 f = (center_dist - clamp(center_dist, -region_range, region_range)) * scale + 0.5;

	FragColor = vec4(texture(Source, (texel_floored + f) * params.SourceSize.zw).rgb, 1.0);
}
