const vec2 positions[4] = vec2[](
    vec2(0.5, -0.5), // Right bottom
    vec2(-0.5, -0.5), // Left bottom
    vec2(0.5, 0.5), // Right top
    vec2(-0.5, 0.5) // Left top
);

// coords = 0.0 to 1.0 texture coordinates
vec2 sheet_texture_coords(vec2 coords, vec2 u, vec2 v) {
    return vec2(mix(u.x, u.y, coords.x+0.5), mix(v.x, v.y, coords.y+0.5));
}

struct UvOffset {
    vec2 u;
    vec2 v;
};

UvOffset sheet_uv(uint sprite_number, uvec2 sprite_dimensions, uvec2 sheet_dimensions) {
    uvec2 sheet_rowcol = uvec2((sheet_dimensions.x / sprite_dimensions.x), (sheet_dimensions.y / sprite_dimensions.y));

    uint row = (sprite_number / sheet_rowcol.x);
    uint col = sprite_number - uint(sheet_rowcol.x * row);

    UvOffset offset;
    offset.u = vec2(float(col * sprite_dimensions.x) / float(sheet_dimensions.x), float((col + 1) * sprite_dimensions.x) / float(sheet_dimensions.x));
    offset.v = vec2(float(row * sprite_dimensions.y) / float(sheet_dimensions.y), float((row + 1) * sprite_dimensions.y) / float(sheet_dimensions.y));

    return offset;
}