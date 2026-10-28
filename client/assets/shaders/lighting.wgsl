// Obscurité du château: une couche sombre sur toute la vue, trouée par des halos lisses.
// `lights[i]` = (x, y, rayon, intensité) en coordonnées du monde; `params` = (nombre de lumières, obscurité ambiante).
#import bevy_sprite::mesh2d_vertex_output::VertexOutput

struct LightData {
    lights: array<vec4<f32>, 16>,
    params: vec4<f32>,
    tint: vec4<f32>,
    view: vec4<f32>,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> data: LightData;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let p = in.world_position.xy;
    var light = 0.0;
    let count = u32(data.params.x);
    for (var i = 0u; i < count; i = i + 1u) {
        let l = data.lights[i];
        let t = clamp(1.0 - distance(p, l.xy) / l.z, 0.0, 1.0);
        // Falloff doux: plein au centre, s'éteint progressivement sur le bord du halo.
        light = max(light, t * t * (3.0 - 2.0 * t) * l.w);
    }
    let darkness = data.params.y * (1.0 - light);
    // Le pourtour de la vue s'assombrit encore, sauf près d'une lumière.
    let vignette = smoothstep(data.view.z, data.view.z + 110.0, distance(p, data.view.xy)) * data.view.w * (1.0 - light);
    let shade = darkness + (1.0 - darkness) * vignette;
    // Pénombre bleutée, ou rouge sombre en mode sanglant (params.z), teintée par la tenue du héros dirigé.
    let base_dark = mix(vec3<f32>(0.02, 0.025, 0.08), vec3<f32>(0.07, 0.0, 0.01), data.params.z);
    let dark = mix(base_dark, data.tint.rgb * 0.24, 0.8);
    // Voile de la couleur du héros sur toute l'image: c'est la colorimétrie de la scène.
    let wash = data.tint.w;
    let rgb = mix(dark, data.tint.rgb, light);
    let alpha = shade + (1.0 - shade) * wash;
    return vec4<f32>(rgb, alpha);
}
