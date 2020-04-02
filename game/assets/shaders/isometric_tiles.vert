#version 450 core

#include "util/coords.glsl"
#include "util/flat.glsl"
#include "util/sprite.glsl"
#include "util/pod.glsl"

layout (location = 0) in uint index;
layout (location = 1) in uint sprite;
layout (location = 2) in vec4 color;

layout(location = 0) out struct { vec4 color; vec2 uv;  } vertex;

void main() {
    vec2 tex_uv = vec2(positions[gl_VertexIndex][0], positions[gl_VertexIndex][1]);
    UvOffset uv = sheet_uv(sprite, props.sprite_dimensions, props.sheet_dimensions);

    vec3 coords = flat_decode(index, props.map_dimensions);
    vec4 origin = coord_to_grid(coords, props.sprite_dimensions, tex_uv);

    vec4 offset = vec4( -(float(props.map_dimensions.x) * float(props.sprite_dimensions.x)) / 2.0,
                        -((float(props.map_dimensions.y) * float(props.sprite_dimensions.y)) / 2.0 ),
                        0.0, 0.0);
    offset = offset + vec4(float(props.sprite_dimensions.x), float(props.sprite_dimensions.y), 0.0, 0.0);

    origin = origin + offset;

    gl_Position = props.view_proj * origin;
    vertex.uv = sheet_texture_coords(tex_uv, uv.u, uv.v);
    vertex.color = color;
}