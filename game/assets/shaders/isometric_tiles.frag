#version 450 core

layout(set=0, binding=1) uniform sampler2D sTexture;

layout(location = 0) in struct {
    vec4 color;
        vec2 uv;
} args;

layout(location = 0) out vec4 fColor;

void main()
{
    if (args.color.a == 0) {
        discard;
    }

    fColor = args.color * texture(sTexture, args.uv);
}