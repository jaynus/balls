layout(set=0, binding=0) uniform SpriteProperties {
    uvec2 sheet_dimensions;
    uvec3 map_dimensions;
    uvec2 sprite_dimensions;
    mat4 view_proj;
    vec3 camera_translation;
} props;