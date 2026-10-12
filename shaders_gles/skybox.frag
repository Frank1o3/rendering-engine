#version 300 es
precision highp float;

uniform vec3 uSkyColor;
uniform vec3 uSunDir;      // normalised direction to the sun
uniform vec3 uSunColor;    // colour of the sun disk
uniform float uSunSize;    // angular radius in radians
uniform float uCloudCoverage;
uniform float uTime;
uniform mat4 uInvVP;
uniform vec3 uCameraPosition;

in vec2 vNdc;
out vec4 FragColor;

float hash31(vec3 p) {
    p = fract(p * 0.1031);
    p += dot(p, p.yzx + 33.33);
    return fract((p.x + p.y) * p.z);
}

float noise3(vec3 p) {
    vec3 cell = floor(p);
    vec3 f = fract(p);
    f = f * f * (3.0 - 2.0 * f);

    float n000 = hash31(cell + vec3(0.0, 0.0, 0.0));
    float n100 = hash31(cell + vec3(1.0, 0.0, 0.0));
    float n010 = hash31(cell + vec3(0.0, 1.0, 0.0));
    float n110 = hash31(cell + vec3(1.0, 1.0, 0.0));
    float n001 = hash31(cell + vec3(0.0, 0.0, 1.0));
    float n101 = hash31(cell + vec3(1.0, 0.0, 1.0));
    float n011 = hash31(cell + vec3(0.0, 1.0, 1.0));
    float n111 = hash31(cell + vec3(1.0, 1.0, 1.0));

    float x00 = mix(n000, n100, f.x);
    float x10 = mix(n010, n110, f.x);
    float x01 = mix(n001, n101, f.x);
    float x11 = mix(n011, n111, f.x);
    return mix(mix(x00, x10, f.y), mix(x01, x11, f.y), f.z);
}

float cloud_noise(vec3 p) {
    float value = 0.0;
    float amplitude = 0.5;
    for (int octave = 0; octave < 4; ++octave) {
        value += noise3(p) * amplitude;
        p = p * 2.03 + vec3(17.1, 9.2, 4.7);
        amplitude *= 0.5;
    }
    return value;
}

void main() {
    // Reconstruct each view ray per pixel. Interpolating rays across the
    // fullscreen triangle bends the sun as the camera turns.
    vec4 world = uInvVP * vec4(vNdc, 0.0, 1.0);
    vec3 direction = normalize(world.xyz / world.w - uCameraPosition);

    // Base sky colour
    vec3 color = uSkyColor;

    // Angle between view direction and sun direction
    float cosAngle = dot(direction, normalize(uSunDir));
    float angle = acos(clamp(cosAngle, -1.0, 1.0));

    // Soft sun disk
    float radius = uSunSize;
    float glow = 1.0 - smoothstep(radius * 0.5, radius * 1.5, angle);
    color = mix(color, uSunColor, glow);

    // A scrolling spherical cloud field avoids the horizon stretching caused
    // by projecting the sky direction onto a flat plane.
    // Set uCloudCoverage to zero to disable it.
    if (uCloudCoverage > 0.001 && direction.y > -0.12) {
        vec3 cloudPosition = direction * 5.0 + vec3(uTime * 0.018, 0.0, uTime * 0.006);
        float density = cloud_noise(cloudPosition);
        float threshold = mix(0.70, 0.46, clamp(uCloudCoverage, 0.0, 1.0));
        float clouds = smoothstep(threshold, threshold + 0.10, density);
        clouds *= smoothstep(-0.12, 0.05, direction.y) * uCloudCoverage;
        color = mix(color, vec3(0.92, 0.95, 1.0), clouds);
    }

    FragColor = vec4(color, 1.0);
}
