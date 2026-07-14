// shaders/wireframe.vert
// Feeds into wireframe.geom which emits barycentric-based edge lines.
// Attribute layout identical to all other shaders — matches geometry_pool.rs.
#version 460 core

layout (location = 0) in vec3  aPos;
layout (location = 1) in vec3  aNormal;
layout (location = 2) in vec4  aColor;

layout (location = 3) in vec3  iPosition;
layout (location = 4) in float iScale;
layout (location = 5) in vec4  iRotation;

out vec4 vColor;

uniform mat4 uVP;

vec3 quat_rotate(vec3 v, vec4 q) {
    vec3 t = cross(q.xyz, v) + q.w * v;
    return v + 2.0 * cross(q.xyz, t);
}

void main() {
    vec3 worldPos = quat_rotate(aPos * iScale, iRotation) + iPosition;
    gl_Position   = uVP * vec4(worldPos, 1.0);
    vColor        = aColor;
}
