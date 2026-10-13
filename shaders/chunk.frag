#version 460

layout(location = 0) in vec3 v_normal;
layout(location = 1) in vec3 v_world_pos;
layout(location = 2) flat in uint v_material;
layout(location = 3) flat in uint v_dir;
layout(location = 4) in vec2 v_uv;
layout(location = 5) in vec3 v_light; // x: ao (0..3), y: sky (0..15), z: block (0..15)

layout(location = 0) out vec4 out_color;

layout(set = 0, binding = 0) uniform sampler2DArray u_textures;
layout(set = 0, binding = 1) uniform sampler2D u_lightmap;

uint get_texture_layer(uint mat, uint dir) {
    switch (mat) {
        case 1u: // Stone
            return 0u;
        case 2u: // Dirt
            return 1u;
        case 3u: // Grass Block
            if (dir == 2u) return 2u; // Top face -> grass_block_top
            if (dir == 3u) return 1u; // Bottom face -> dirt
            return 3u;                // Side faces -> grass_block_side
        case 4u: // Bedrock
            return 4u;
        case 5u: // Sand
            return 5u;
        case 7u: // Oak Planks
            return 6u;
        case 21u: // logic_power_block
            return 17u;
        case 24u: // logic_lamp
            return 18u;
        case 25u: // logic_lamp_lit
            return 19u;
        case 35u: // Cobblestone
            return 47u;
        case 36u: // Obsidian
            return 48u;
        case 51u: // Coal Ore
            return 49u;
        case 52u: // Iron Ore
            return 50u;
        case 53u: // Copper Ore
            return 51u;
        case 54u: // Gold Ore
            return 52u;
        case 55u: // Redstone Ore
            return 53u;
        case 56u: // Lapis Ore
            return 54u;
        case 57u: // Diamond Ore
            return 55u;
        case 58u: // Emerald Ore
            return 56u;
        case 59u: // Deepslate
            if (dir == 2u || dir == 3u) return 58u;
            return 57u;
        case 60u: // Deepslate Coal Ore
            return 59u;
        case 61u: // Deepslate Iron Ore
            return 60u;
        case 62u: // Deepslate Copper Ore
            return 61u;
        case 63u: // Deepslate Gold Ore
            return 62u;
        case 64u: // Deepslate Redstone Ore
            return 63u;
        case 65u: // Deepslate Lapis Ore
            return 64u;
        case 66u: // Deepslate Diamond Ore
            return 65u;
        case 67u: // Deepslate Emerald Ore
            return 66u;
        case 68u: // Granite
            return 67u;
        case 69u: // Diorite
            return 68u;
        case 70u: // Andesite
            return 69u;
        case 71u: // Tuff
            return 70u;
        case 72u: // Raw Iron Block
            return 71u;
        case 73u: // Raw Copper Block
            return 72u;
        case 74u: // Oak Log
            if (dir == 2u || dir == 3u) return 25u; // Top/Bottom -> oak_log_top
            return 24u;                             // Sides -> oak_log
        case 75u: // Birch Log
            if (dir == 2u || dir == 3u) return 27u; // Top/Bottom -> birch_log_top
            return 26u;                             // Sides -> birch_log
        case 76u: // Spruce Log
            if (dir == 2u || dir == 3u) return 29u; // Top/Bottom -> spruce_log_top
            return 28u;                             // Sides -> spruce_log
        case 79u: // Mossy Cobblestone
            return 73u;
        case 80u: // Monster Spawner
            return 74u;
        case 82u: // Furnace
            if (dir == 2u || dir == 3u) return 78u;
            if (dir == 4u) return 79u;
            return 81u;
        case 83u: // Lit Furnace
            if (dir == 2u || dir == 3u) return 78u;
            if (dir == 4u) return 80u;
            return 81u;
        case 84u: // Crafting Table
            if (dir == 2u) return 75u;
            if (dir == 3u) return 6u;
            if (dir == 4u || dir == 5u) return 76u;
            return 77u;
        case 90u: // Anvil
            if (dir == 2u) return 83u;
            return 82u;
        case 95u: // Netherrack
            return 37u;
        case 96u: // Soul Sand
            return 38u;
        case 97u: // Glowstone
            return 39u;
        case 98u: // Basalt
            if (dir == 2u || dir == 3u) return 88u;
            return 40u;
        case 99u: // Blackstone
            if (dir == 2u) return 89u;
            return 41u;
        case 100u: // Magma Block
            return 42u;
        case 101u: // Soul Soil
            return 43u;
        case 102u: // Nether Quartz Ore
            return 44u;
        case 103u: // Nether Gold Ore
            return 45u;
        case 104u: // Ancient Debris
            if (dir == 2u || dir == 3u) return 90u;
            return 46u;
        default:
            return 0u;
    }
}

void main() {
    uint layer = get_texture_layer(v_material, v_dir);

    // Sample repeating 2D texture array slice
    vec4 tex_color = texture(u_textures, vec3(v_uv, float(layer)));

    // Biome tint for grass top (plains green tint #79c05a)
    if (v_material == 3u && v_dir == 2u) {
        tex_color.rgb *= vec3(0.474, 0.753, 0.353);
    }

    // Directional face shading factor
    float face_shade = 0.75;
    if (v_normal.y > 0.5) {
        face_shade = 1.0;
    } else if (v_normal.y < -0.5) {
        face_shade = 0.5;
    } else if (abs(v_normal.z) > 0.5) {
        face_shade = 0.85;
    } else {
        face_shade = 0.75;
    }

    // Unpack smooth interpolated light components
    float ao_raw = v_light.x;    // 0.0 .. 3.0
    float sky_raw = v_light.y;   // 0.0 .. 15.0
    float block_raw = v_light.z; // 0.0 .. 15.0

    // Smooth ambient occlusion factor
    float ao_factor = mix(0.35, 1.0, ao_raw / 3.0);

    // Sample 16x16 lightmap LUT with bilinear filtering
    vec2 lightmap_uv = clamp(vec2(block_raw + 0.5, sky_raw + 0.5) / 16.0, 0.0, 1.0);
    vec3 sampled_light = texture(u_lightmap, lightmap_uv).rgb;

    // Daylight ambient floor: prevents outdoor side faces from turning pitch black
    vec3 sky_day_color = texture(u_lightmap, vec2(0.5 / 16.0, 15.5 / 16.0)).rgb;
    vec3 daylight_floor = sky_day_color * 0.20;
    float sky_exposure = clamp(sky_raw / 2.0, 0.0, 1.0);
    vec3 light_color = mix(sampled_light, max(sampled_light, daylight_floor), sky_exposure);

    vec3 total_light = clamp(light_color * face_shade * ao_factor, 0.0, 1.0);

    // Emissive block components
    if (v_material == 25u || v_material == 97u) {
        total_light = max(total_light, vec3(1.0, 0.92, 0.75));
    } else if (v_material == 21u) {
        total_light = max(total_light, vec3(0.9, 0.3, 0.3));
    } else if (v_material == 100u || (v_material == 83u && v_dir == 4u)) {
        total_light = max(total_light, vec3(0.9, 0.6, 0.3));
    }

    out_color = vec4(tex_color.rgb * total_light, tex_color.a);
}
