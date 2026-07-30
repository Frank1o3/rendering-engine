#version 320 es
precision highp float;

in  vec4 fColor;
in  vec3 fWorldNormal;
in  vec3 gBary;
out vec4 FragColor;

uniform vec3 uSunDir;
uniform float uAmbient;
uniform float uWireframe;

void main() {
    vec3  n        = normalize(fWorldNormal);
    float diffuse  = max(dot(n, uSunDir), 0.0);
    float lighting = uAmbient + (1.0 - uAmbient) * diffuse;
    vec3  litColor = fColor.rgb * lighting;

    if (uWireframe > 0.5) {
        float d = min(gBary.x, min(gBary.y, gBary.z));
        float edgeFactor = smoothstep(0.0, fwidth(d) * 1.5, d);
        vec3 wireColor = vec3(1.0, 1.0, 0.0);
        FragColor = vec4(mix(wireColor, litColor, edgeFactor), fColor.a);
    } else {
        FragColor = vec4(litColor, fColor.a);
    }
}
