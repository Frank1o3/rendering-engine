// shaders/ui.vert
// Orthographic 2-D overlay.  Attribute layout must match geometry_pool.rs.
#version 460 core

layout (location = 0) in vec3  aPos;
layout (location = 1) in vec3  aNormal;  // unused for 2-D UI
layout (location = 2) in vec4  aColor;

layout (location = 3) in vec3  iPosition;
layout (location = 4) in float iScale;
layout (location = 5) in vec4  iRotation; // unused for 2-D UI

out vec4 ourColor;

uniform mat4 uVP;

void main() {
    // Scale the unit quad, then translate to pixel position.
    // Rotation is intentionally ignored for flat UI elements.
    vec3 transformed = aPos * iScale + iPosition;
    gl_Position = uVP * vec4(transformed.xy, 0.0, 1.0);
    ourColor = aColor;
}
