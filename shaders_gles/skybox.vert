#version 300 es
precision highp float;

uniform mat4 uInvVP;
out vec3 vDir;

void main() {
    float x = -1.0 + float((gl_VertexID & 1) << 2);
    float y = -1.0 + float((gl_VertexID & 2) << 1);
    gl_Position = vec4(x, y, 0.0, 1.0);
    vec4 world = uInvVP * vec4(x, y, 0.0, 1.0);
    vDir = normalize(world.xyz);
}
