#version 450 core

layout(set=0, binding=1) uniform sampler2D texture;

layout(location = 0) in struct {
    vec4 color;
    vec2 uv;
} vertex;

layout(location = 0) out vec4 fColor;

void main() {
    if (args.color.a == 0) {
        discard;
    }

    vec4 texColor = texture2D(texture, vertex.uv);
    fColor = texColor * vertex.color;
}