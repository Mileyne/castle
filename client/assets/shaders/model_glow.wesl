// Matériau des modèles 3D (héros, ennemis, objets): le matériau standard de Bevy, plus une émission qui reprend la couleur du modèle. La
// couleur finale d'un point (sommets, tissage des vêtements, peau du visage) en émet une part `glow.x`, quelle que soit la lumière reçue:
// le modèle brille de ses propres couleurs, sans se délaver. `glow.x` = 0: le matériau standard, inchangé.
import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT,
};

@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> glow: vec4<f32>;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(
        pbr_input.material.flags,
        pbr_input.material.alpha_cutoff,
        pbr_input.material.base_color,
    );
    // L'émission est la couleur du modèle en ce point, multipliée par le réglage. Son alpha, qui pèse l'exposition de la caméra, est à 0 comme
    // pour le matériau standard (sinon l'émission serait réduite d'un facteur mille).
    pbr_input.material.emissive = vec4<f32>(pbr_input.material.base_color.rgb * glow.x, 0.0);
    var out: FragmentOutput;
    if (pbr_input.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        out.color = apply_pbr_lighting(pbr_input);
    } else {
        out.color = pbr_input.material.base_color;
    }
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
