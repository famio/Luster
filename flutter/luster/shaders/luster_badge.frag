// A badge lit by its studio's panorama alone: flutter_scene's standard
// image-based lighting (the split-sum specular with multiple-scattering
// energy, the diffuse spherical harmonics, the specular anti-aliasing of a
// normal-mapped surface), with nothing else. The standard shader also carries
// the analytic and punctual lights, shadows, ambient occlusion, an irradiance
// field, fog and a second environment; even switched off they cost a phone
// GPU most of its time, and this shader is several times quicker. The terms it
// keeps are the standard shader's, line for line, so a badge looks the same
// drawn either way.

#include <material_varyings.glsl>
#include <normals.glsl>
#include <pbr.glsl>
#include <texture.glsl>
#include <diffuse_sh.glsl>

uniform sampler2D base_color_texture;
uniform sampler2D normal_texture;
// The environment's diffuse spherical harmonics, as the engine stores them.
uniform sampler2D irradiance_sh;
uniform sampler2D brdf_lut;
uniform RadianceSampler prefiltered_radiance;

uniform BadgeInfo {
  // Linear base colour, and alpha.
  vec4 color;
  // Dielectric F0 in xyz.
  vec4 dielectric_f0;
  // Metallic, perceptual roughness, normal scale, whether there is a normal
  // map.
  vec4 surface;
  // Environment intensity, specular anti-aliasing variance and threshold.
  vec4 environment;
  mat4 environment_transform;
}
badge;

// The standard shader's SpecularAARoughness.
float SpecularAntiAliasedRoughness(vec3 normal, float roughness) {
  if (badge.environment.y <= 0.0) {
    return roughness;
  }
  vec3 d_normal_x = dFdx(normal);
  vec3 d_normal_y = dFdy(normal);
  float variance = badge.environment.y *
                   max(dot(d_normal_x, d_normal_x), dot(d_normal_y, d_normal_y));
  float kernel = min(2.0 * variance, badge.environment.z);
  float square_roughness =
      clamp(roughness * roughness + kernel, kMinRoughness * kMinRoughness, 1.0);
  return sqrt(square_roughness);
}

void main() {
  vec4 base_color_srgb = texture(base_color_texture, v_texture_coords);
  vec3 albedo = SRGBToLinear(base_color_srgb.rgb) * badge.color.rgb;
  float alpha = base_color_srgb.a * badge.color.a;

  vec3 geometric_normal = GetWorldNormal();
  vec3 normal = geometric_normal;
  if (badge.surface.w > 0.5) {
    normal = PerturbNormal(normal_texture, normal, v_viewvector,
                           v_texture_coords, badge.surface.z);
  }
  float metallic = clamp(badge.surface.x, 0.0, 1.0);
  float roughness = SpecularAntiAliasedRoughness(
      normal, clamp(badge.surface.y, kMinRoughness, 1.0));

  vec3 camera_normal = normalize(v_viewvector);
  vec3 reflectance = mix(badge.dielectric_f0.xyz, albedo, metallic);
  // The Fresnel and the split-sum energy take the geometric normal, the
  // reflection the perturbed one (see the standard shader).
  float n_dot_v_energy = max(dot(geometric_normal, camera_normal), 0.0);
  vec3 reflection_normal = reflect(-camera_normal, normal);
  vec3 k_S = FresnelSchlickRoughness(n_dot_v_energy, reflectance, roughness);

  mat3 environment_transform = mat3(badge.environment_transform);
  vec3 irradiance =
      max(EvaluateDiffuseSH(irradiance_sh, environment_transform * normal, 0),
          vec3(0.0)) *
      badge.environment.x;
  vec3 prefiltered_color =
      SampleRadianceEnv(prefiltered_radiance,
                        environment_transform * reflection_normal, roughness) *
      badge.environment.x;

  vec2 f_ab = texture(brdf_lut,
                      vec2(clamp(n_dot_v_energy, 0.0, 0.99) / 3.0,
                           clamp(roughness, 0.0, 0.99)))
                  .rg;
  vec3 FssEss = k_S * f_ab.x + f_ab.y;
  float Ems = 1.0 - (f_ab.x + f_ab.y);
  vec3 F_avg = reflectance + (1.0 - reflectance) / 21.0;
  vec3 FmsEms = Ems * FssEss * F_avg / (1.0 - F_avg * Ems);
  vec3 k_D = albedo * (1.0 - metallic) * (1.0 - FssEss + FmsEms);

  vec3 color = FssEss * prefiltered_color + (FmsEms + k_D) * irradiance;
  // Linear HDR, premultiplied by alpha, as the resolve pass expects.
  frag_color = vec4(color, 1.0) * alpha;
}
