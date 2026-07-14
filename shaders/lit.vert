// shaders/lit.vert
// Lambertian diffuse lighting in world space.
// Attribute layout identical to basic.vert — must match geometry_pool.rs.
#version 460 core

layout (location = 0) in vec3  aPos;
layout (location = 1) in vec3  aNormal;   // packed i8 normalised → [-1,1] vec3
layout (location = 2) in vec4  aColor;

layout (location = 3) in vec3  iPosition;
layout (location = 4) in float iScale;
layout (location = 5) in vec4  iRotation;

out vec4 vColor;
out vec3 vWorldNormal;

uniform mat4 uVP;

vec3 quat_rotate(vec3 v, vec4 q) {
    vec3 t = cross(q.xyz, v) + q.w * v;
    return v + 2.0 * cross(q.xyz, t);
}

void main() {
    vec3 worldPos    = quat_rotate(aPos * iScale, iRotation) + iPosition;
    // Normals don't scale — just rotate
    vec3 worldNormal = normalize(quat_rotate(aNormal, iRotation));

    gl_Position  = uVP * vec4(worldPos, 1.0);
    vColor       = aColor;
    vWorldNormal = worldNormal;
}
