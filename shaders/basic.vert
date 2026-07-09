// shaders/basic.vert
#version 460 core

layout (location = 0) in vec3 aPos;
layout (location = 1) in vec4 aColor;

// Instanced transform attributes
layout (location = 2) in vec3 iPosition;
layout (location = 3) in float iScale;
layout (location = 4) in vec4 iRotation; // Quaternion (x, y, z, w)

out vec4 ourColor;

// Global View * Projection matrix
uniform mat4 uVP;

// Rotates vector v by quaternion q (xyzw)
vec3 quat_rotate(vec3 v, vec4 q) {
    vec3 temp = cross(q.xyz, v) + q.w * v;
    return v + 2.0 * cross(q.xyz, temp);
}

void main() {
    // Scale, then rotate by quaternion, then translate
    vec3 transformed = quat_rotate(aPos * iScale, iRotation) + iPosition;
    gl_Position = uVP * vec4(transformed, 1.0);
    ourColor = aColor;
}
