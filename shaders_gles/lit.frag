#version 320 es
precision highp float;

in  vec4 fColor;
in  vec3 fWorldNormal;
in  vec3 fWorldPosition;
in  vec2 fUV;
in  vec3 gBary;
out vec4 FragColor;

uniform float uAmbient;
uniform float uWireframe;
uniform vec3 uMaterialAlbedo;
uniform sampler2D uAlbedoTexture;
uniform int uHasAlbedoTexture;

struct EngineLight {
    vec3 position_or_dir; uint light_type;
    vec3 color; float intensity;
    vec3 spot_direction; float range;
    float spot_angle_cos; float ambient_influence;
    uint casts_shadow; float specular_intensity;
};
layout(std140) uniform EngineLights {
    uint uLightCount;
    uint _lightPadding0;
    uint _lightPadding1;
    uint _lightPadding2;
    EngineLight uLights[16];
};

void main() {
    vec3  n        = normalize(fWorldNormal);
    vec3 illumination = vec3(uAmbient);
    for (uint i = 0u; i < uLightCount; ++i) {
        EngineLight light = uLights[i];
        vec3 toLight;
        float attenuation = 1.0;
        if (light.light_type == 0u) {
            toLight = normalize(light.position_or_dir);
        } else {
            vec3 offset = light.position_or_dir - fWorldPosition;
            float distanceToLight = length(offset);
            toLight = distanceToLight > 0.0 ? offset / distanceToLight : vec3(0.0, 1.0, 0.0);
            float rangeFade = clamp(1.0 - distanceToLight / max(light.range, 0.0001), 0.0, 1.0);
            attenuation = rangeFade * rangeFade;
            if (light.light_type == 2u) {
                float cone = dot(normalize(-toLight), normalize(light.spot_direction));
                attenuation *= smoothstep(light.spot_angle_cos, min(1.0, light.spot_angle_cos + 0.05), cone);
            }
        }
        float diffuse = max(dot(n, toLight), 0.0);
        illumination += light.color * light.intensity * diffuse * attenuation;
    }
    vec4 baseColor = fColor;
    if (uHasAlbedoTexture != 0) {
        baseColor *= texture(uAlbedoTexture, fUV);
    }
    vec3 litColor = baseColor.rgb * uMaterialAlbedo * illumination;

    if (uWireframe > 0.5) {
        float d = min(gBary.x, min(gBary.y, gBary.z));
        float edgeFactor = smoothstep(0.0, fwidth(d) * 1.5, d);
        vec3 wireColor = vec3(1.0, 1.0, 0.0);
        FragColor = vec4(mix(wireColor, litColor, edgeFactor), baseColor.a);
    } else {
        FragColor = vec4(litColor, baseColor.a);
    }
}
