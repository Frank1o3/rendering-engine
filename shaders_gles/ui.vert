#version 320 es
precision highp float;

layout (location = 0) in vec3  aPos;
layout (location = 1) in vec3  aNormal;
layout (location = 2) in vec4  aColor;

layout (location = 3) in vec3  iPosition;
layout (location = 4) in float iScale;
layout (location = 5) in vec4  iRotation;

out vec4 ourColor;

uniform mat4 uVP;

void main() {
    vec3 transformed = aPos * iScale + iPosition;
    gl_Position = uVP * vec4(transformed.xy, 0.0, 1.0);
    ourColor = aColor;
}
