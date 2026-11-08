//! The Sets column's goals (.superpowers/ui-proof/sets-column/goals.md): each goal's shortest path, ending in its check.

journey!(
    g1_open_a_set_from_its_card,
    "demo",
    [
        r#"{"op":"click","at":{"label":"armor/gold"}}"#,
        r#"{"op":"check","name":"one_set_open"}"#
    ]
);

journey!(
    g2_show_only_the_sets_that_need_work,
    "demo",
    [
        r#"{"op":"click","at":{"starts":"Not ready"}}"#,
        r#"{"op":"expect","fact":"/counts/visible_sets","equals":3}"#,
        r#"{"op":"check","name":"sets_pills_match_their_counts"}"#,
        r#"{"op":"check","name":"sets_not_ready_shows_exactly_problem_sets"}"#
    ]
);

journey!(
    g3_show_only_the_sets_that_write_no_json,
    "demo",
    [
        r#"{"op":"click","at":{"starts":"No JSON"}}"#,
        r#"{"op":"expect","fact":"/counts/visible_sets","equals":7}"#,
        r#"{"op":"check","name":"sets_pills_match_their_counts"}"#,
        r#"{"op":"check","name":"sets_no_json_shows_exactly_json_less_sets"}"#
    ]
);

journey!(
    g4_find_sets_by_typing_part_of_their_name,
    "demo",
    [
        r#"{"op":"type","text":"STONE","at":{"placeholder":"Search sets"}}"#,
        r#"{"op":"expect","fact":"/counts/visible_sets","equals":3}"#,
        r#"{"op":"check","name":"sets_search_shows_exactly_name_matches"}"#
    ]
);

journey!(
    g5_narrow_the_sets_to_one_shader,
    "demo",
    [
        r#"{"op":"click","at":{"label":"Sets shader filter"}}"#,
        r#"{"op":"click","at":{"label":"PBR 2"}}"#,
        r#"{"op":"expect","fact":"/counts/visible_sets","equals":2}"#,
        r#"{"op":"check","name":"sets_shader_shows_exactly_that_shader"}"#
    ]
);

journey!(
    g6_keep_the_open_set_while_a_filter_hides_it,
    "demo",
    [
        r#"{"op":"click","at":{"label":"armor/gold"}}"#,
        r#"{"op":"click","at":{"starts":"Not ready"}}"#,
        r#"{"op":"expect","fact":"/sets/armor~1gold/visible","equals":false}"#,
        r#"{"op":"expect","fact":"/selection","equals":"set:armor/gold"}"#,
        r#"{"op":"check","name":"selected_set_kept_while_hidden"}"#,
        r#"{"op":"click","at":{"label":"Clear filter"}}"#,
        r#"{"op":"expect","fact":"/sets/armor~1gold/visible","equals":true}"#
    ]
);

journey!(
    g8_show_only_the_ready_sets,
    "demo",
    [
        r#"{"op":"click","at":{"starts":"Ready"}}"#,
        r#"{"op":"check","name":"sets_ready_shows_exactly_clean_sets"}"#
    ]
);

journey!(
    g9_filter_parts_combine_with_and,
    "demo",
    [
        r#"{"op":"click","at":{"starts":"Not ready"}}"#,
        r#"{"op":"type","text":"stone","at":{"placeholder":"Search sets"}}"#,
        r#"{"op":"check","name":"sets_filter_shows_the_and_of_its_parts"}"#
    ]
);

journey!(
    g7_open_one_map_of_a_set_from_its_slot_chip,
    "demo",
    [
        r#"{"op":"click","at":{"label":"RMAOS","index":0}}"#,
        r#"{"op":"expect","fact":"/selection","equals":"item:architecture/stone/cobble_rmaos.png"}"#
    ]
);

journey!(
    d1_a_badge_on_a_filled_chip_opens_its_helper,
    "helpers",
    [
        r#"{"op":"type","text":"crate","at":{"placeholder":"Search sets"}}"#,
        r#"{"op":"click","at":{"label":"crate_opacity.png, fills D alpha"}}"#,
        r#"{"op":"expect","fact":"/selection","equals":"item:props/crate_opacity.png"}"#
    ]
);

journey!(
    d2_a_badge_on_the_packed_rmaos_chip_opens_its_helper,
    "helpers",
    [
        r#"{"op":"type","text":"crate","at":{"placeholder":"Search sets"}}"#,
        r#"{"op":"click","at":{"label":"crate_roughness.png, fills RMAOS R"}}"#,
        r#"{"op":"expect","fact":"/selection","equals":"item:props/crate_roughness.png"}"#
    ]
);

journey!(
    d3_a_helper_that_fills_two_chips_has_a_badge_on_each,
    "helpers",
    [
        r#"{"op":"type","text":"crate","at":{"placeholder":"Search sets"}}"#,
        r#"{"op":"click","at":{"label":"crate_roughness.png, fills CNR alpha"}}"#,
        r#"{"op":"expect","fact":"/selection","equals":"item:props/crate_roughness.png"}"#,
        r#"{"op":"click","at":{"label":"props/crate"}}"#,
        r#"{"op":"expect","fact":"/selection","equals":"set:props/crate"}"#,
        r#"{"op":"click","at":{"label":"crate_roughness.png, fills RMAOS R"}}"#,
        r#"{"op":"expect","fact":"/selection","equals":"item:props/crate_roughness.png"}"#
    ]
);

journey!(
    d4_an_unused_helper_stays_in_the_trailing_row,
    "helpers",
    [
        r#"{"op":"type","text":"crate","at":{"placeholder":"Search sets"}}"#,
        r#"{"op":"click","at":{"label":"crate_metallic.png, not used: no targets chosen"}}"#,
        r#"{"op":"expect","fact":"/selection","equals":"item:props/crate_metallic.png"}"#
    ]
);

journey!(
    g10_the_roughness_helper_is_found_from_its_badge,
    "helpers",
    [
        r#"{"op":"type","text":"crate","at":{"placeholder":"Search sets"}}"#,
        r#"{"op":"click","at":{"label":"crate_roughness.png, fills RMAOS R"}}"#,
        r#"{"op":"expect","fact":"/selection","equals":"item:props/crate_roughness.png"}"#
    ]
);
