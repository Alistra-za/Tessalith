//! The Sets column's goals (.superpowers/ui-proof/sets-column/goals.md): each goal's shortest path, ending in its check.

journey!(
    g1_open_a_set_from_its_card,
    "demo",
    [
        r#"{"op":"click","at":{"label":"armor/gold"}}"#,
        r#"{"op":"expect","fact":"/selection","equals":"set:armor/gold"}"#
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
        r#"{"op":"click","at":{"label":"Filter ▾","index":1}}"#,
        r#"{"op":"click","at":{"label":"PBR"}}"#,
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
        r#"{"op":"click","at":{"label":"Clear filter"}}"#,
        r#"{"op":"expect","fact":"/sets/armor~1gold/visible","equals":true}"#,
        r#"{"op":"check","name":"selected_set_kept_while_hidden"}"#
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
