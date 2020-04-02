#version 450 core
//#extension GL_ARB_separate_shader_objects : enable

#include "util/coords.glsl"
#include "util/flat.glsl"
#include "util/sprite.glsl"
#include "util/pod.glsl"
#include "desc/sprite_properties.glsl"

layout (location = 0) in uint index;
layout (location = 1) in uint sprite;
layout (location = 2) in vec4 color;

layout (location = 0) out vec4          out_color;
layout (location = 1) out vec2          out_uv;
layout (location = 3) flat out uint     out_sprite;

const vec2 lol[4] = vec2[](
vec2(1.0, 0.0), // Right bottom
vec2(0.0, 0.0), // Left bottom
vec2(1.0, 1.0), // Right top
vec2(0.0, 1.0) // Left top
);

void main() {
    vec2 tex_uv = vec2(lol[gl_VertexIndex][0], lol[gl_VertexIndex][1]);

    vec3 coords = flat_decode(index, props.map_dimensions);
    vec4 origin = coord_to_grid(coords, props.sprite_dimensions, tex_uv);

    vec4 offset = vec4( -(float(props.map_dimensions.x) * float(props.sprite_dimensions.x)) / 2.0,
                        -((float(props.map_dimensions.y) * float(props.sprite_dimensions.y)) / 2.0 ),
                        0.0, 0.0);
    //offset = offset + vec4(float(props.sprite_dimensions.x)/2, float(props.sprite_dimensions.y)/2, 0.0, 0.0);

    origin = origin + offset;

    gl_Position = props.view_proj * origin;

    out_uv = tex_uv;
    out_color = color;
    out_sprite = sprite;
}