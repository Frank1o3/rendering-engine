// shaders/ui.vert
#version 460 core

layout (location = 0) in vec3 aPos;
layout (location = 1) in vec4 aColor;

// Instanced transform attributes (bound to locations 2, 3, 4 to match Mesh VAO layout)
layout (location = 2) in vec3 iPosition;
layout (location = 3) in float iScale;
layout (location = 4) in vec4 iRotation; // Quaternion, unused for 2D UI

out vec4 ourColor;

// Orthographic UI projection matrix
uniform mat4 uVP;

void main() {
    // Scale the 1x1 quad, then translate to absolute pixel position
    vec3 transformed = aPos * iScale + iPosition;
    gl_Position = uVP * vec4(transformed.xy, 0.0, 1.0);
    ourColor = aColor;
}
