#version 450

/*
    CRT-interlaced ("CRT Geom")

    Copyright (C) 2010-2012 cgwg, Themaister and DOLLS

    This program is free software; you can redistribute it and/or modify it
    under the terms of the GNU General Public License as published by the Free
    Software Foundation; either version 2 of the License, or (at your option)
    any later version.

    (cgwg gave their consent to have the original version of this shader
    distributed under the GPL in this message:

    http://board.byuu.org/viewtopic.php?p=26075#p26075

    "Feel free to distribute my shaders under the GPL. After all, the
    barrel distortion code was taken from the Curvature shader, which is
    under the GPL."
    )

    Ported from the copy of crt-geom.slang that OpenEmu ships, as a single fragment
    shader with its settings fixed at the defaults (screen curvature on, dot mask 0.3,
    scanline weight 0.3). Changes from that copy:
      - what its vertex stage worked out once (the angles and the "stretch" that
        fits the curved screen into the window) are constants here; "stretch" was
        evaluated for these settings with the original maxscale()
      - the interlace simulation and vertical scanlines options are left out; the
        interlace simulation needs a frame counter
      - the screen has the 4:3 aspect ratio the original intends. The slang copy
        writes it with a comma operator, which makes both axes 0.75 instead
*/

layout(push_constant) uniform Push
{
	vec4 SourceSize;
	vec4 OutputSize;
} params;

layout(location = 0) in vec2 vUV;
layout(location = 0) out vec4 FragColor;
layout(set = 0, binding = 0) uniform sampler2D Source;

const float CRTgamma = 2.4;
const float monitorgamma = 2.2;
// distance of the viewer from the screen, and the radius of the screen's curve
const float d = 1.5;
const float R = 2.0;
const float cornersize = 0.03;
const float cornersmooth = 1000.0;
const float DOTMASK = 0.3;
const float scanline_weight = 0.3;
const float lum = 0.0;
const vec2 overscan = vec2(1.0, 1.0);
const vec2 aspect = vec2(1.0, 0.75);
// tilt of the screen: none
const vec2 sinangle = vec2(0.0);
const vec2 cosangle = vec2(1.0);
// maxscale() for these settings
const vec3 stretch = vec3(0.0, 0.0, 0.9714492);

#define FIX(c) max(abs(c), 1e-5)
#define PI 3.141592653589
// interpolation is done in linear light
#define TEX2D(c) pow(texture(Source, (c)), vec4(CRTgamma))

float intersect(vec2 xy)
{
	float A = dot(xy, xy) + d * d;
	float B = 2.0 * (R * (dot(xy, sinangle) - d * cosangle.x * cosangle.y) - d * d);
	float C = d * d + 2.0 * R * d * cosangle.x * cosangle.y;

	return (-B - sqrt(B * B - 4.0 * A * C)) / (2.0 * A);
}

vec2 bkwtrans(vec2 xy)
{
	float c = intersect(xy);
	vec2 point = (vec2(c, c) * xy - vec2(-R, -R) * sinangle) / vec2(R, R);
	vec2 poc = point / cosangle;
	vec2 tang = sinangle / cosangle;

	float A = dot(tang, tang) + 1.0;
	float B = -2.0 * dot(poc, tang);
	float C = dot(poc, poc) - 1.0;

	float a = (-B + sqrt(B * B - 4.0 * A * C)) / (2.0 * A);
	vec2 uv = (point - a * sinangle) / cosangle;
	float r = FIX(R * acos(a));

	return uv * r / sin(r / R);
}

// Calculate the influence of a scanline on the current pixel.
//
// 'distance' is the distance in texture coordinates from the current
// pixel to the scanline in question.
// 'color' is the colour of the scanline at the horizontal location of
// the current pixel.
vec4 scanlineWeights(float distance, vec4 color)
{
	// "wid" controls the width of the scanline beam, for each RGB
	// channel. The "weights" lines specify the formula that gives the
	// profile of the beam, i.e. the intensity as a function of distance
	// from the vertical center of the scanline.
	vec4 wid = 2.0 + 2.0 * pow(color, vec4(4.0));
	vec4 weights = vec4(distance / scanline_weight);

	return (lum + 1.4) * exp(-pow(weights * inversesqrt(0.5 * wid), wid)) / (0.6 + 0.2 * wid);
}

vec2 transform(vec2 coord)
{
	coord = (coord - vec2(0.5, 0.5)) * aspect * stretch.z + stretch.xy;

	return bkwtrans(coord) / overscan / aspect + vec2(0.5, 0.5);
}

float corner(vec2 coord)
{
	coord = (coord - vec2(0.5)) * overscan + vec2(0.5, 0.5);
	coord = min(coord, vec2(1.0) - coord) * aspect;
	vec2 cdist = vec2(cornersize);
	coord = (cdist - min(coord, cdist));
	float dist = sqrt(dot(coord, coord));

	return clamp((cdist.x - dist) * cornersmooth, 0.0, 1.0);
}

void main()
{
	// Here's a helpful diagram to keep in mind while trying to
	// understand the code:
	//
	//  |      |      |      |      |
	// -------------------------------
	//  |      |      |      |      |
	//  |  01  |  11  |  21  |  31  | <-- current scanline
	//  |      | @    |      |      |
	// -------------------------------
	//  |      |      |      |      |
	//  |  02  |  12  |  22  |  32  | <-- next scanline
	//  |      |      |      |      |
	// -------------------------------
	//  |      |      |      |      |
	//
	// Each character-cell represents a pixel on the output
	// surface, "@" represents the current pixel (always somewhere
	// in the bottom half of the current scan-line, or the top-half
	// of the next scanline). The grid of lines represents the
	// edges of the texels of the underlying texture.

	vec2 TextureSize = params.SourceSize.xy;
	// The size of one texel, in texture-coordinates.
	vec2 one = 1.0 / TextureSize;

	// Texture coordinates of the texel containing the active pixel.
	vec2 xy = transform(vUV * vec2(1.00001));

	float cval = corner(xy);

	// Of all the pixels that are mapped onto the texel we are
	// currently rendering, which pixel are we currently rendering?
	vec2 ratio_scale = xy * TextureSize - vec2(0.5, 0.5);
	vec2 uv_ratio = fract(ratio_scale);

	// Snap to the center of the underlying texel.
	xy = (floor(ratio_scale) + vec2(0.5, 0.5)) / TextureSize;

	// Calculate Lanczos scaling coefficients describing the effect
	// of various neighbour texels in a scanline on the current
	// pixel.
	vec4 coeffs = PI * vec4(1.0 + uv_ratio.x, uv_ratio.x, 1.0 - uv_ratio.x, 2.0 - uv_ratio.x);

	// Prevent division by zero.
	coeffs = FIX(coeffs);

	// Lanczos2 kernel.
	coeffs = 2.0 * sin(coeffs) * sin(coeffs / 2.0) / (coeffs * coeffs);

	// Normalize.
	coeffs /= dot(coeffs, vec4(1.0));

	// Calculate the effective colour of the current and next
	// scanlines at the horizontal location of the current pixel,
	// using the Lanczos coefficients above.
	vec4 col = clamp(
		mat4(
			TEX2D(xy + vec2(-one.x, 0.0)),
			TEX2D(xy),
			TEX2D(xy + vec2(one.x, 0.0)),
			TEX2D(xy + vec2(2.0 * one.x, 0.0))
		) * coeffs,
		0.0, 1.0
	);
	vec4 col2 = clamp(
		mat4(
			TEX2D(xy + vec2(-one.x, one.y)),
			TEX2D(xy + vec2(0.0, one.y)),
			TEX2D(xy + one),
			TEX2D(xy + vec2(2.0 * one.x, one.y))
		) * coeffs,
		0.0, 1.0
	);

	// Calculate the influence of the current and next scanlines on
	// the current pixel. Three samples across the beam each, which
	// reduces the moire caused by scanlines and curvature.
	vec4 weights = scanlineWeights(uv_ratio.y, col);
	vec4 weights2 = scanlineWeights(1.0 - uv_ratio.y, col2);

	float filter_ = fwidth(ratio_scale.y);
	uv_ratio.y = uv_ratio.y + 1.0 / 3.0 * filter_;
	weights = (weights + scanlineWeights(uv_ratio.y, col)) / 3.0;
	weights2 = (weights2 + scanlineWeights(abs(1.0 - uv_ratio.y), col2)) / 3.0;
	uv_ratio.y = uv_ratio.y - 2.0 / 3.0 * filter_;
	weights = weights + scanlineWeights(abs(uv_ratio.y), col) / 3.0;
	weights2 = weights2 + scanlineWeights(abs(1.0 - uv_ratio.y), col2) / 3.0;

	vec3 mul_res = (col * weights + col2 * weights2).rgb * vec3(cval);

	// dot-mask emulation:
	// Output pixels are alternately tinted green and magenta.
	vec3 dotMaskWeights = mix(
		vec3(1.0, 1.0 - DOTMASK, 1.0),
		vec3(1.0 - DOTMASK, 1.0, 1.0 - DOTMASK),
		floor(mod(gl_FragCoord.x, 2.0))
	);

	mul_res *= dotMaskWeights;

	// Convert the image gamma for display on our output device.
	mul_res = pow(mul_res, vec3(1.0 / monitorgamma));

	FragColor = vec4(mul_res, 1.0);
}
