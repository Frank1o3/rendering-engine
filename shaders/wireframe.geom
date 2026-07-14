// shaders/wireframe.geom
// Receives triangles, emits barycentric coordinates so the fragment shader can
// draw crisp edges without a second geometry pass.
// Each triangle vertex gets one of (1,0,0), (0,1,0), (0,0,1) as its
// barycentric weight; the fragment discards pixels far from any edge.
#version 460 core

layout (triangles) in;
layout (triangle_strip, max_vertices = 3) out;

in  vec4 vColor[];
out vec4 gColor;
out vec3 gBary;   // barycentric coordinates

void main() {
    gl_Position = gl_in[0].gl_Position;
    gColor = vColor[0];
    gBary  = vec3(1.0, 0.0, 0.0);
    EmitVertex();

    gl_Position = gl_in[1].gl_Position;
    gColor = vColor[1];
    gBary  = vec3(0.0, 1.0, 0.0);
    EmitVertex();

    gl_Position = gl_in[2].gl_Position;
    gColor = vColor[2];
    gBary  = vec3(0.0, 0.0, 1.0);
    EmitVertex();

    EndPrimitive();
}
