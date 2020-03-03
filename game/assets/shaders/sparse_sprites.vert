#version 450 core

#include "util/flat.glsl"
#include "util/sprite.glsl"
#include "util/pod.glsl"

layout(set=0, binding=0) uniform Properties {
    mat4 view_proj;
} props;

layout(location = 0) in vec3 pos;
layout(location = 1) in vec2 u_offset;
layout(location = 2) in vec2 v_offset;
layout(location = 3) in vec4 color;
layout(location = 4) in vec2 dir_x;
layout(location = 5) in vec2 dir_y;

layout(location = 0) out struct { vec4 color; vec2 uv; } vertex;

void main() {
    float tex_u = positions[gl_VertexIndex][0];
    float tex_v = positions[gl_VertexIndex][1];

    vec2 final_pos = pos.xy + tex_u * dir_x + tex_v * dir_y;
    vec4 origin = vec4(final_pos, pos.z, 1.0);

    gl_Position = props.view_proj * origin;
    vertex.uv = sheet_texture_coords(vec2(tex_u, tex_v), u_offset, v_offset);
    vertex.color = color;
}