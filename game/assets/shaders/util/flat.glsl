
uint flat_encode(uint x, uint y, uint z, uvec3 map_dimensions) {
    return ((z * map_dimensions.x * map_dimensions.y) + (y * map_dimensions.x) + x);
}


uvec3 flat_decode(uint index, uvec3 map_dimensions) {
    uint z = index / (map_dimensions.x * map_dimensions.y);
    uint idx = index - (z * map_dimensions.x * map_dimensions.y);
    uint y = idx / map_dimensions.x;
    uint x = idx % map_dimensions.x;

    return uvec3(x, y, z);
}