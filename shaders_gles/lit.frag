#version 320 es
precision highp float;

in  vec4 vColor;
in  vec3 vWorldNormal;
out vec4 FragColor;

uniform vec3 uSunDir;
uniform float uAmbient;

void main() {
    vec3  n        = normalize(vWorldNormal);
    float diffuse  = max(dot(n, uSunDir), 0.0);
    float lighting = uAmbient + (1.0 - uAmbient) * diffuse;
    FragColor = vec4(vColor.rgb * lighting, vColor.a);
}
