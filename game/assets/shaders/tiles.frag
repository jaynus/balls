#version 450 core

layout(set=0, binding=1) uniform sampler2D sTexture[512];

layout (location = 0) in vec4          color;
layout (location = 1) in vec2          uv;
layout (location = 3) flat in uint    sprite;


layout(location = 0) out vec4 fColor;

void main()
{
    if (color.a == 0) {
        discard;
    }

    fColor = color * texture(sTexture[sprite], uv);
}