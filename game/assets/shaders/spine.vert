#version 450 core

#include "util/flat.glsl"
#include "util/sprite.glsl"
#include "util/pod.glsl"
#include "desc/sprite_properties.glsl"

layout (location = 0) in vec2 a_position;
layout (location = 1) in vec2 a_texCoords;
layout (location = 2) in vec4 a_color;

uniform mat4 view_proj;

layout(location = 0) out struct { vec4 color; vec2 uv; } vertex;

void main() {
    vertex.color = a_color;
    vertex.uv = a_texCoords;

    gl_Position = props.view_proj * vec4(a_position, 0.0, 1.0);
}