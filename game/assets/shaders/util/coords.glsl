vec4 coord_to_isometric(vec3 map_coordinate, uvec2 sprite_dimensions, vec2 tex_uv) {
    vec2 half_dimensions = vec2(float(sprite_dimensions.x / 2), float(sprite_dimensions.x / 2));
    vec4 origin = vec4(
        map_coordinate.x * half_dimensions.x - map_coordinate.y * half_dimensions.x,
        map_coordinate.x * half_dimensions.y + map_coordinate.y * half_dimensions.y,
        1.0,
        1.0
    );
    return origin + vec4((tex_uv.x * float(sprite_dimensions.x)), (tex_uv.y * float(sprite_dimensions.y)), 0.0, 0.0);
}

vec4 coord_to_grid(vec3 map_coordinate, uvec2 sprite_dimensions, vec2 tex_uv) {
    return vec4(
        float(sprite_dimensions.x) * map_coordinate.x + (tex_uv.x * float(sprite_dimensions.x)),
        float(sprite_dimensions.y) * map_coordinate.y + (tex_uv.y * float(sprite_dimensions.y)),
        1.0,
        1.0
    );
}