#version 300 es
precision highp float;

uniform vec3 uSkyColor;
uniform vec3 uSunDir;      // normalised direction to the sun
uniform vec3 uSunColor;    // colour of the sun disk
uniform float uSunSize;    // angular radius in radians

in vec3 vDir;
out vec4 FragColor;

void main() {
    // Base sky colour
    vec3 color = uSkyColor;

    // Angle between view direction and sun direction
    float cosAngle = dot(normalize(vDir), normalize(uSunDir));
    float angle = acos(clamp(cosAngle, -1.0, 1.0));

    // Soft sun disk
    float radius = uSunSize;
    float glow = 1.0 - smoothstep(radius * 0.5, radius * 1.5, angle);
    color = mix(color, uSunColor, glow);

    FragColor = vec4(color, 1.0);
}
