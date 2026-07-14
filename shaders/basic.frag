// shaders/basic.frag
#version 460 core

in  vec4 ourColor;
out vec4 FragColor;

void main() {
    FragColor = ourColor;
}
