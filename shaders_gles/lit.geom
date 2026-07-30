#version 320 es
precision highp float;

layout(triangles) in;
layout(triangle_strip, max_vertices = 3) out;

in vec4 vColor[];
in vec3 vWorldNormal[];

out vec4 fColor;
out vec3 fWorldNormal;
out vec3 gBary;

void main() {
    vec3 baryCoords[3] = vec3[3](
        vec3(1.0, 0.0, 0.0),
        vec3(0.0, 1.0, 0.0),
        vec3(0.0, 0.0, 1.0)
    );

    for (int i = 0; i < 3; i++) {
        gl_Position   = gl_in[i].gl_Position;
        fColor        = vColor[i];
        fWorldNormal  = vWorldNormal[i];
        gBary         = baryCoords[i];
        EmitVertex();
    }
    EndPrimitive();
}
