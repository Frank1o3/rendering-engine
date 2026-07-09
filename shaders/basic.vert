// shaders/basic.vert
#version 460 core

layout (location = 0) in vec3 aPos;
layout (location = 1) in vec4 aColor;

// Instanced model matrix (automatically offset by base_instance)
layout (location = 2) in mat4 aModel;

out vec4 ourColor;

// Global View * Projection matrix
uniform mat4 uVP;

void main() {
    // The GPU applies the per-instance model matrix!
    gl_Position = uVP * aModel * vec4(aPos, 1.0);
    ourColor = aColor;
}
