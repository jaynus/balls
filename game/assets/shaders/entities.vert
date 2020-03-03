#version 450 core

#include "util/flat.glsl"
#include "util/sprite.glsl"
#include "util/pod.glsl"
#include "desc/sprite_properties.glsl"

layout (location = 0) in vec3 pos;
layout (location = 1) in uint sprite;
layout (location = 2) in vec4 color;

layout(location = 0) out struct { vec4 color; vec2 uv; } vertex;

void main() {
    vec2 tex_uv = vec2(positions[gl_VertexIndex][0], positions[gl_VertexIndex][1]);
    UvOffset uv = sheet_uv(sprite, props.sprite_dimensions, props.sheet_dimensions);

    vec3 camera_adjusted_origin = vec3(pos.x, pos.y, pos.z - props.camera_translation.z);
    if (int(floor(camera_adjusted_origin.z)) != 0) {
        vertex.color.a = 0;
        return;
    }
    vec4 origin = vec4(
        camera_adjusted_origin.x + float(props.sprite_dimensions.x/2) + (tex_uv.x * float(props.sprite_dimensions.x)),
        camera_adjusted_origin.y + float(props.sprite_dimensions.y/2) + (tex_uv.y * float(props.sprite_dimensions.y)),
        2.0,
        1.0
    );

    gl_Position = props.view_proj * origin;
    vertex.uv = sheet_texture_coords(tex_uv, uv.u, uv.v);
    vertex.color = color;
}