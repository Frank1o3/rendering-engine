// shaders/basic.vert
// Attribute layout must match geometry_pool.rs exactly:
//   location 0 — aPos      vec3  f32   (position)
//   location 1 — aNormal   vec3  i8    (normal, normalised — unused in this shader)
//   location 2 — aColor    vec4  u8    (per-vertex color, normalised)
//   location 3 — iPosition vec3  f32   (instance position)
//   location 4 — iScale    float f32   (instance uniform scale)
//   location 5 — iRotation vec4  f32   (instance quaternion xyzw)
#version 460 core

layout (location = 0) in vec3 aPos;
layout (location = 1) in vec3 aNormal;   // normalised on upload, not used here
layout (location = 2) in vec4 aColor;

// Instanced transform attributes
layout (location = 3) in vec3  iPosition;
layout (location = 4) in float iScale;
layout (location = 5) in vec4  iRotation; // quaternion (x, y, z, w)

out vec4 ourColor;

uniform mat4 uVP;

// Rotates vector v by unit quaternion q (xyzw)
vec3 quat_rotate(vec3 v, vec4 q) {
    vec3 t = cross(q.xyz, v) + q.w * v;
    return v + 2.0 * cross(q.xyz, t);
}

void main() {
    vec3 worldPos = quat_rotate(aPos * iScale, iRotation) + iPosition;
    gl_Position = uVP * vec4(worldPos, 1.0);
    ourColor = aColor;
}
