// shaders/lit.frag
// Simple Lambertian diffuse + ambient.
// uSunDir should be normalised before upload.
#version 460 core

in  vec4 vColor;
in  vec3 vWorldNormal;
out vec4 FragColor;

uniform vec3 uSunDir;   // direction *toward* the light (world space, normalised)
uniform float uAmbient; // ambient intensity in [0, 1]

void main() {
    vec3  n        = normalize(vWorldNormal);
    float diffuse  = max(dot(n, uSunDir), 0.0);
    float lighting = uAmbient + (1.0 - uAmbient) * diffuse;
    FragColor = vec4(vColor.rgb * lighting, vColor.a);
}
