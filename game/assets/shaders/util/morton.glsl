// Z-curve / Morton code
// refs : https://en.wikipedia.org/wiki/Z-order_curve
//        https://fgiesen.wordpress.com/2009/12/13/decoding-morton-codes/

uint MASKS[] = uint[] (0x55555555, 0x33333333, 0x0F0F0F0F, 0x00FF00FF, 0x0000FFFF);

uint morton_encode_2d(vec2 U) {       // --- grid location to curve index
    uvec2 I = uvec2(U);
    uint n=8;

    for (uint i=3; i>=0; i--) {
        I =  (I | (I << n)) & MASKS[i], n /= 2;
    }

    return I.x | (I.y << 1);
}

uvec2 morton_decode_2d(uint z) {      // --- curve index to grid location
    uint n=1;
    uvec2 I = uvec2(z,z>>1) & MASKS[0];

    for (uint i=1; i<=4; i++) {
        I = (I | (I >>  n)) & MASKS[i], n *= 2;
    }

    return I;
}