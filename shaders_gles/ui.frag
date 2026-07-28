#version 320 es
precision highp float;

in  vec4 ourColor;
out vec4 FragColor;

void main() {
    FragColor = ourColor;
}
