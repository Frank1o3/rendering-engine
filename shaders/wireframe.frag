// shaders/wireframe.frag
// Renders solid faces with bright-white edges drawn using barycentric coordinates.
// Edge thickness is controlled by uEdgeWidth (screen-space in barycentric units).
#version 460 core

in  vec4 gColor;
in  vec3 gBary;
out vec4 FragColor;

uniform float uEdgeWidth; // try 0.03 – 0.06

void main() {
    // Minimum distance to any edge in barycentric space.
    float minBary = min(gBary.x, min(gBary.y, gBary.z));

    // Smooth step from edge to interior.
    float edge = 1.0 - smoothstep(uEdgeWidth - 0.005, uEdgeWidth + 0.005, minBary);

    // Blend between the face color and white edge color.
    vec4 faceColor = vec4(gColor.rgb * 0.35, gColor.a); // darken face so edges pop
    vec4 edgeColor = vec4(1.0, 1.0, 1.0, 1.0);
    FragColor = mix(faceColor, edgeColor, edge);
}
